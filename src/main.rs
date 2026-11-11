slint::include_modules!();

mod torrent;

use std::error::Error;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Unset WAYLAND_DISPLAY so Winit connects via XWayland where KWin allows coordinate manipulation
    std::env::remove_var("WAYLAND_DISPLAY");
    std::env::set_var("WINIT_UNIX_BACKEND", "x11");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to initialize tracing logger");

    info!("NetCassette MPRIS system media listener armed! (ง ͠° ͟ل͜ ͡°)ง");

    // Инициализация P2P BitTorrent движка в папке Музыки
    let music_dir = directories::UserDirs::new()
        .and_then(|u| u.audio_dir().map(|p| p.join("NetCassette")))
        .unwrap_or_else(|| std::path::PathBuf::from("./music"));

    let _torrent_engine = match torrent::TorrentEngine::new(&music_dir).await {
        Ok(engine) => {
            info!(path = ?music_dir, "P2P BitTorrent swarm engine initialized (★ω★)");
            Some(std::sync::Arc::new(engine))
        }
        Err(err) => {
            tracing::warn!(error = ?err, "Failed to start torrent engine");
            None
        }
    };

    let app = MainWindow::new()?;
    let app_handle = app.as_weak();

    let playlist_win = PlaylistWindow::new()?;
    let playlist_handle = playlist_win.as_weak();

    // Command bridge to send UI clicks to Linux MPRIS players
    enum MprisCmd { PlayPause, Stop, Next, Previous }
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<MprisCmd>();

    // Hover sensor logger
    app.on_hover_changed({
        move |is_hovered| {
            info!(is_hovered, "Widget hover state changed (•‿•)");
        }
    });

    // Detect screen dimensions for window placement
    let (screen_width, screen_height) = std::process::Command::new("xrandr")
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines().find(|l| l.contains(" connected primary") || l.contains(" connected")).and_then(|l| {
                let chunk = l.split_whitespace().find(|w| w.contains('x') && w.contains('+'))?;
                let res = chunk.split('+').next()?;
                let mut parts = res.split('x');
                let w = parts.next()?.parse::<f32>().ok()?;
                let h = parts.next()?.parse::<f32>().ok()?;
                Some((w, h))
            })
        })
        .unwrap_or((1920.0, 1080.0));

    // Persistent configuration structure
    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct AppConfig {
        y: f32,
        width: f32,
        height: f32,
    }

    impl Default for AppConfig {
        fn default() -> Self {
            Self { y: 20.0, width: 360.0, height: 270.0 }
        }
    }

    let config_dir = directories::BaseDirs::new()
        .map(|b| b.config_dir().join("netcassette"))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let _ = std::fs::create_dir_all(&config_dir);
    let config_path = config_dir.join("config.json");

    let saved_config: AppConfig = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default();

    let config_state = std::sync::Arc::new(std::sync::Mutex::new(saved_config.clone()));

    // Apply saved dimensions to UI
    app.set_base_width(saved_config.width);
    app.set_base_height(saved_config.height);

    let save_cfg = {
        let path = config_path.clone();
        let state = config_state.clone();
        move || {
            if let Ok(cfg) = state.lock() {
                if let Ok(data) = serde_json::to_string_pretty(&*cfg) {
                    let _ = std::fs::write(&path, data);
                }
            }
        }
    };

    // Fluid LMB vertical slide handler with persistent Y saving
    let save_on_move = save_cfg.clone();
    let state_on_move = config_state.clone();
    app.on_move_window_y({
        let handle = app_handle.clone();
        move |dy| {
            if let Some(ui) = handle.upgrade() {
                let current_pos = ui.window().position();
                let next_y = (current_pos.y + dy as i32).max(0);
                ui.window().set_position(slint::PhysicalPosition::new(current_pos.x, next_y));

                if let Ok(mut cfg) = state_on_move.lock() {
                    cfg.y = next_y as f32;
                }
                save_on_move();
            }
        }
    });

    // Dynamic corner resizing handler (adjusts dimensions and re-anchors to right screen edge)
    let save_on_resize = save_cfg.clone();
    let state_on_resize = config_state.clone();
    app.on_resize_window({
        let handle = app_handle.clone();
        move |dw, dh| {
            if let Some(ui) = handle.upgrade() {
                let cur_w = ui.get_base_width();
                let cur_h = ui.get_base_height();

                let new_w = (cur_w + dw).max(280.0).min(650.0);
                let new_h = (cur_h + dh).max(200.0).min(750.0);

                ui.set_base_width(new_w);
                ui.set_base_height(new_h);

                // Re-anchor to right edge with new width
                let pos = ui.window().position();
                let new_x = screen_width - new_w;
                ui.window().set_position(slint::PhysicalPosition::new(new_x as i32, pos.y));

                if let Ok(mut cfg) = state_on_resize.lock() {
                    cfg.width = new_w;
                    cfg.height = new_h;
                }
                save_on_resize();
            }
        }
    });

    // Clean exit from menu
    app.on_quit_app(|| {
        info!("Exiting NetCassette background widget ( ˘ω˘ )ﾉ");
        std::process::exit(0);
    });

    // Forward Play/Pause to system player
    let tx_play = cmd_tx.clone();
    app.on_play_clicked(move || {
        let _ = tx_play.send(MprisCmd::PlayPause);
    });

    // Forward Stop to system player
    let tx_stop = cmd_tx.clone();
    app.on_stop_clicked(move || {
        let _ = tx_stop.send(MprisCmd::Stop);
    });

    // Forward Previous to system player
    let tx_prev = cmd_tx.clone();
    app.on_prev_clicked(move || {
        let _ = tx_prev.send(MprisCmd::Previous);
    });

    // Forward Next to system player
    let tx_next = cmd_tx.clone();
    app.on_next_clicked(move || {
        let _ = tx_next.send(MprisCmd::Next);
    });

    // Background Tokio task: Live MPRIS synchronization & command dispatch
    let mpris_ui_handle = app_handle.clone();
    let mpris_playlist_handle = playlist_handle.clone();
    tokio::spawn(async move {
        let Ok(conn) = zbus::Connection::session().await else {
            tracing::warn!("Could not connect to session D-Bus for MPRIS");
            return;
        };

        let mut last_title = String::new();
        let mut last_artist = String::new();
        let mut last_art_url = String::new();
        let mut last_status = false;

        loop {
            // Check for incoming user button commands
            while let Ok(cmd) = cmd_rx.try_recv() {
                if let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await {
                    if let Ok(names) = dbus.list_names().await {
                        if let Some(player) = names.iter().find(|n| n.as_str().starts_with("org.mpris.MediaPlayer2.")) {
                            if let Ok(proxy) = zbus::Proxy::new(&conn, player.as_str(), "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player").await {
                                let _ = match cmd {
                                    MprisCmd::PlayPause => proxy.call::<_, _, ()>("PlayPause", &()).await,
                                    MprisCmd::Stop => proxy.call::<_, _, ()>("Stop", &()).await,
                                    MprisCmd::Next => proxy.call::<_, _, ()>("Next", &()).await,
                                    MprisCmd::Previous => proxy.call::<_, _, ()>("Previous", &()).await,
                                };
                            }
                        }
                    }
                }
            }

            // Sync currently playing media from the first active player
            if let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await {
                if let Ok(names) = dbus.list_names().await {
                    let active_player = names.iter().find(|n| n.as_str().starts_with("org.mpris.MediaPlayer2."));
                    
                    if let Some(player_name) = active_player {
                        if let Ok(proxy) = zbus::Proxy::new(&conn, player_name.as_str(), "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player").await {
                            let status: String = proxy.get_property("PlaybackStatus").await.unwrap_or_default();
                            let is_playing = status == "Playing";

                            let meta: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
                                proxy.get_property("Metadata").await.unwrap_or_default();

                            use std::ops::Deref;
                            let extract_meta = |key: &str| -> Option<String> {
                                match meta.get(key)?.deref() {
                                    zbus::zvariant::Value::Str(s) => Some(s.as_str().to_string()),
                                    zbus::zvariant::Value::Array(arr) => {
                                        let items: Vec<String> = arr.iter().filter_map(|item| {
                                            if let zbus::zvariant::Value::Str(s) = item {
                                                Some(s.as_str().to_string())
                                            } else {
                                                None
                                            }
                                        }).collect();
                                        if items.is_empty() { None } else { Some(items.join(", ")) }
                                    }
                                    _ => None,
                                }
                            };

                            let title = extract_meta("xesam:title")
                                .unwrap_or_else(|| "No Media Title".into());

                            let artist = extract_meta("xesam:artist")
                                .unwrap_or_else(|| player_name.as_str().replace("org.mpris.MediaPlayer2.", ""));

                            let art_url = extract_meta("mpris:artUrl").unwrap_or_default();

                            if title != last_title || artist != last_artist || is_playing != last_status || art_url != last_art_url {
                                last_title = title.clone();
                                last_artist = artist.clone();
                                last_art_url = art_url.clone();
                                last_status = is_playing;

                                // Resolve path in background thread safely (Send-compatible)
                                let cover_path: Option<std::path::PathBuf> = if !art_url.is_empty() {
                                    if art_url.starts_with("file://") {
                                        let file_path = art_url.trim_start_matches("file://");
                                        Some(std::path::PathBuf::from(file_path))
                                    } else if art_url.starts_with("http://") || art_url.starts_with("https://") {
                                        if let Ok(resp) = reqwest::get(&art_url).await {
                                            if let Ok(bytes) = resp.bytes().await {
                                                let temp_cover = std::env::temp_dir().join("netcassette_cover_cache.img");
                                                if std::fs::write(&temp_cover, bytes).is_ok() {
                                                    Some(temp_cover)
                                                } else {
                                                    None
                                                }
                                            } else { None }
                                        } else { None }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                let handle = mpris_ui_handle.clone();
                                let pl_sync_handle = mpris_playlist_handle.clone();
                                let title_for_pl = title.clone();
                                let artist_for_pl = artist.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    let img = cover_path.as_ref().and_then(|p| slint::Image::load_from_path(p).ok());

                                    if let Some(ui) = handle.upgrade() {
                                        ui.set_track_title(title.into());
                                        ui.set_track_artist(artist.into());
                                        ui.set_is_playing(is_playing);
                                        if let Some(ref cover) = img {
                                            ui.set_track_cover(cover.clone());
                                        }
                                    }
                                    if let Some(pl) = pl_sync_handle.upgrade() {
                                        pl.set_track_title(title_for_pl.into());
                                        pl.set_track_artist(artist_for_pl.into());
                                        pl.set_is_playing(is_playing);
                                        if let Some(ref cover) = img {
                                            pl.set_track_cover(cover.clone());
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        }
    });

    use std::sync::atomic::{AtomicBool, Ordering};
    let playlist_is_open = std::sync::Arc::new(AtomicBool::new(false));

    // Меню кассеты: открыть / скрыть окно по центру
    let pl_toggle_handle = playlist_handle.clone();
    let open_state_toggle = playlist_is_open.clone();
    app.on_toggle_menu(move || {
        if let Some(pl) = pl_toggle_handle.upgrade() {
            let was_open = open_state_toggle.load(Ordering::SeqCst);
            let next_state = !was_open;
            open_state_toggle.store(next_state, Ordering::SeqCst);

            if next_state {
                let win_w = 680.0;
                let win_h = 440.0;
                let center_x = ((screen_width - win_w) / 2.0).max(0.0);
                let center_y = ((screen_height - win_h) / 2.0).max(0.0);
                pl.window().set_position(slint::LogicalPosition::new(center_x, center_y));
                let _ = pl.show();
                info!("Playlist window opened ( ＾◡＾)っ");
            } else {
                let _ = pl.hide();
                info!("Playlist window hidden (˘ω˘)");
            }
        }
    });

    // Крестик в окне
    let pl_close_handle = playlist_handle.clone();
    let open_state_close = playlist_is_open.clone();
    playlist_win.on_close_clicked(move || {
        if let Some(pl) = pl_close_handle.upgrade() {
            let _ = pl.hide();
            open_state_close.store(false, Ordering::SeqCst);
            info!("Playlist window closed via cross button (•‿•)");
        }
    });

    // Открытие папки музыки в проводнике KDE / Dolphin
    playlist_win.on_open_music_folder_clicked(|| {
        let music_dir = directories::UserDirs::new()
            .and_then(|u| u.audio_dir().map(|p| p.join("NetCassette")))
            .unwrap_or_else(|| std::path::PathBuf::from("./music"));
        let _ = std::fs::create_dir_all(&music_dir);
        let _ = std::process::Command::new("xdg-open").arg(&music_dir).spawn();
        info!("Opened NetCassette music folder (•‿•)");
    });

    // Кнопки управления треком из окна плейлиста
    let tx_pl_play = cmd_tx.clone();
    playlist_win.on_play_clicked(move || {
        let _ = tx_pl_play.send(MprisCmd::PlayPause);
    });

    let tx_pl_prev = cmd_tx.clone();
    playlist_win.on_prev_clicked(move || {
        let _ = tx_pl_prev.send(MprisCmd::Previous);
    });

    let tx_pl_next = cmd_tx.clone();
    playlist_win.on_next_clicked(move || {
        let _ = tx_pl_next.send(MprisCmd::Next);
    });

    let pl_torrent_engine = _torrent_engine.clone();
    let pl_window_handle = playlist_handle.clone();
    playlist_win.on_add_magnet_clicked(move || {
        if let Some(pl) = pl_window_handle.upgrade() {
            let magnet = pl.get_magnet_input().to_string();
            if !magnet.is_empty() {
                if let Some(ref engine) = pl_torrent_engine {
                    let eng = engine.clone();
                    let mag = magnet.clone();
                    tokio::spawn(async move {
                        match eng.add_magnet(&mag).await {
                            Ok(msg) => info!(msg, "Magnet link added successfully (•‿•)"),
                            Err(e) => tracing::error!(error = ?e, "Failed to add magnet link"),
                        }
                    });
                }
                pl.set_magnet_input("".into());
            }
        }
    });

    let pl_seed_engine = _torrent_engine.clone();
    playlist_win.on_seed_music_clicked(move || {
        if let Some(ref engine) = pl_seed_engine {
            let eng = engine.clone();
            let music_dir = directories::UserDirs::new()
                .and_then(|u| u.audio_dir().map(|p| p.join("NetCassette")))
                .unwrap_or_else(|| std::path::PathBuf::from("./music"));

            tokio::spawn(async move {
                if let Ok(mut entries) = tokio::fs::read_dir(&music_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if path.is_file() {
                            match eng.add_path_for_seeding(&path).await {
                                Ok(magnet) => {
                                    info!(?path, %magnet, "Started seeding song! Copy magnet to VM (ง ͠° ͟ل͜ ͡°)ง");
                                    let _ = std::process::Command::new("xclip").args(["-selection", "clipboard"]).stdin(std::process::Stdio::piped()).spawn().and_then(|mut child| {
                                        use std::io::Write;
                                        if let Some(ref mut stdin) = child.stdin {
                                            let _ = stdin.write_all(magnet.as_bytes());
                                        }
                                        Ok(())
                                    });
                                    break;
                                }
                                Err(e) => tracing::error!(error = ?e, "Failed to seed file"),
                            }
                        }
                    }
                }
            });
        }
    });

    let swarms_ui_handle = playlist_handle.clone();
    let swarms_engine = _torrent_engine.clone();
    let swarms_timer = slint::Timer::default();
    swarms_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || {
        if let Some(ref engine) = swarms_engine {
            if let Some(pl) = swarms_ui_handle.upgrade() {
                let stats_vec = engine.list_swarms();
                let mut mapped = Vec::new();
                for s in stats_vec {
                    mapped.push(SlintTorrentItem {
                        id: s.id as i32,
                        name: s.name.into(),
                        progress: s.progress,
                        is_finished: s.is_finished,
                        live_peers: s.live_peers as i32,
                    });
                }
                pl.set_swarms_list(std::rc::Rc::new(slint::VecModel::from(mapped)).into());
            }
        }
    });

    // Force KDE KWin to treat this window as a system desktop dock
    let apply_kde_dock_rules = || {
        // 1. Set X11 DOCK window type so Plasma taskbar removes the icon immediately
        let _ = std::process::Command::new("xprop")
            .args([
                "-name", "NetCassette",
                "-f", "_NET_WM_WINDOW_TYPE", "32a",
                "-set", "_NET_WM_WINDOW_TYPE", "_NET_WM_WINDOW_TYPE_DOCK"
            ])
            .status();

        let _ = std::process::Command::new("xprop")
            .args([
                "-name", "NetCassette",
                "-f", "_NET_WM_STATE", "32a",
                "-set", "_NET_WM_STATE",
                "_NET_WM_STATE_SKIP_TASKBAR, _NET_WM_STATE_SKIP_PAGER, _NET_WM_STATE_ABOVE, _NET_WM_STATE_STICKY"
            ])
            .status();

        let _ = std::process::Command::new("wmctrl")
            .args(["-r", "NetCassette", "-b", "add,skip_taskbar,skip_pager,above,sticky"])
            .status();

        // 2. Register native KWin rule into KDE configuration
        for cmd in &["kwriteconfig6", "kwriteconfig5"] {
            let _ = std::process::Command::new(cmd).args(["--file", "kwinrulesrc", "--group", "NetCassetteDock", "--key", "wmclass", "netcassette"]).status();
            let _ = std::process::Command::new(cmd).args(["--file", "kwinrulesrc", "--group", "NetCassetteDock", "--key", "wmclassmatch", "1"]).status();
            let _ = std::process::Command::new(cmd).args(["--file", "kwinrulesrc", "--group", "NetCassetteDock", "--key", "skiptaskbar", "true"]).status();
            let _ = std::process::Command::new(cmd).args(["--file", "kwinrulesrc", "--group", "NetCassetteDock", "--key", "skiptaskbarrule", "2"]).status();
            let _ = std::process::Command::new(cmd).args(["--file", "kwinrulesrc", "--group", "NetCassetteDock", "--key", "above", "true"]).status();
            let _ = std::process::Command::new(cmd).args(["--file", "kwinrulesrc", "--group", "NetCassetteDock", "--key", "aboverule", "2"]).status();
        }

        // Notify KWin to reload rules
        let _ = std::process::Command::new("qdbus6").args(["org.kde.KWin", "/KWin", "reconfigure"]).status();
        let _ = std::process::Command::new("qdbus").args(["org.kde.KWin", "/KWin", "reconfigure"]).status();
    };

    // Apply rules with retry to catch the X11 surface right when KWin maps it
    let dock_handle = app_handle.clone();
    let initial_y = saved_config.y;
    let initial_w = saved_config.width;
    slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
        if let Some(ui) = dock_handle.upgrade() {
            let target_x = screen_width - initial_w;
            ui.window().set_position(slint::LogicalPosition::new(target_x, initial_y));
            apply_kde_dock_rules();
            info!(target_x, initial_y, "KDE Dock restored at saved height (ﾉ´ヮ`)ﾉ*: ･ﾟ");
        }
    });

    slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
        apply_kde_dock_rules();
    });

    // 7-second cooldown auto-hide controller
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    let last_hover_time = Arc::new(Mutex::new(Instant::now()));
    let hover_track = last_hover_time.clone();
    let auto_hide_handle = app_handle.clone();

    let auto_hide_timer = slint::Timer::default();
    auto_hide_timer.start(slint::TimerMode::Repeated, Duration::from_millis(150), move || {
        if let Some(ui) = auto_hide_handle.upgrade() {
            let is_hovered = ui.get_has_hover();
            let mut last_seen = hover_track.lock().unwrap();

            if is_hovered {
                *last_seen = Instant::now();
                if !ui.get_is_revealed() {
                    ui.set_is_revealed(true);
                }
            } else if ui.get_is_revealed() && last_seen.elapsed() >= Duration::from_secs(7) {
                ui.set_is_revealed(false);
                info!("7 seconds cooldown elapsed: tucked into edge dock (˘ω˘)");
            }
        }
    });

    app.run()?;
    Ok(())
}