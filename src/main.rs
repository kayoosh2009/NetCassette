slint::include_modules!();

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

    let app = MainWindow::new()?;
    let app_handle = app.as_weak();

    // Command bridge to send UI clicks to Linux MPRIS players
    enum MprisCmd { PlayPause, Stop, Next, Previous }
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<MprisCmd>();

    // Hover sensor logger
    app.on_hover_changed({
        move |is_hovered| {
            info!(is_hovered, "Widget hover state changed (•‿•)");
        }
    });

    // Manual Shift + Drag positioning handler
    app.on_move_window({
        let handle = app_handle.clone();
        move |dx, dy| {
            if let Some(ui) = handle.upgrade() {
                let current_pos = ui.window().position();
                let next_x = current_pos.x + dx as i32;
                let next_y = current_pos.y + dy as i32;
                ui.window().set_position(slint::PhysicalPosition::new(next_x, next_y));
            }
        }
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
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = handle.upgrade() {
                                        ui.set_track_title(title.into());
                                        ui.set_track_artist(artist.into());
                                        ui.set_is_playing(is_playing);
                                        if let Some(path) = cover_path {
                                            if let Ok(img) = slint::Image::load_from_path(&path) {
                                                ui.set_track_cover(img);
                                            }
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

    app.on_toggle_menu({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                let open = !ui.get_menu_open();
                ui.set_menu_open(open);
                info!(menu_open = open, "Menu toggled");
            }
        }
    });

    // Detect screen resolution using xrandr / drm
    let screen_width = std::process::Command::new("xrandr")
        .output()
        .ok()
        .and_then(|out| {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines().find(|l| l.contains(" connected primary") || l.contains(" connected")).and_then(|l| {
                let chunk = l.split_whitespace().find(|w| w.contains('x') && w.contains('+'))?;
                let res = chunk.split('+').next()?;
                let w = res.split('x').next()?.parse::<f32>().ok()?;
                Some(w)
            })
        })
        .or_else(|| {
            std::fs::read_dir("/sys/class/drm").ok()?.find_map(|e| {
                let content = std::fs::read_to_string(e.ok()?.path().join("modes")).ok()?;
                let first = content.lines().next()?;
                first.split('x').next()?.parse::<f32>().ok()
            })
        })
        .unwrap_or(1920.0);

    // Dock firmly into KDE right corner right after window is registered
    let dock_handle = app_handle.clone();
    slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
        if let Some(ui) = dock_handle.upgrade() {
            let target_x = screen_width - 360.0;
            let target_y = 0.0;

            let pos_before = ui.window().position();
            info!(?pos_before, "Window position before set_position request");

            ui.window().set_position(slint::LogicalPosition::new(target_x, target_y));

            let pos_after = ui.window().position();
            info!(?pos_after, target_x, target_y, "Window position after set_position request");

            // If KWin ignored Winit set_position, try kdotool / xdotool
            let actual_x = pos_after.x as f32;
            if (actual_x - target_x).abs() > 50.0 {
                info!(actual_x, target_x, "KWin ignored standard positioning, attempting xdotool fallback... (¬_¬)");
                let _ = std::process::Command::new("xdotool")
                    .args(["search", "--name", "NetCassette", "windowmove", &target_x.to_string(), "0"])
                    .status();
            }
        }
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