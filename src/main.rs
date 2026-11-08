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

    info!("NetCassette XWayland shift-drag engine armed! (ง ͠° ͟ل͜ ͡°)ง");

    let app = MainWindow::new()?;
    let app_handle = app.as_weak();

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

    app.on_play_clicked({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                let next = !ui.get_is_playing();
                ui.set_is_playing(next);
                info!(playing = next, "Play / Pause clicked");
            }
        }
    });

    app.on_stop_clicked({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                ui.set_is_playing(false);
                info!("Playback stopped");
            }
        }
    });

    app.on_prev_clicked({
        let handle = app_handle.clone();
        move || {
            let _ = handle;
            info!("Previous track requested");
        }
    });

    app.on_next_clicked({
        let handle = app_handle.clone();
        move || {
            let _ = handle;
            info!("Next track requested");
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