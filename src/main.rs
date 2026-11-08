slint::include_modules!();

use std::error::Error;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to initialize tracing logger");

    info!("NetCassette mini-bar starting... (b ᵔ▽ᵔ)b");

    let app = MainWindow::new()?;
    let app_handle = app.as_weak();

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

    // Detect screen width under Linux and dock window to the right corner
    let screen_width = std::fs::read_dir("/sys/class/drm")
        .ok()
        .and_then(|mut entries| {
            entries.find_map(|entry| {
                let path = entry.ok()?.path().join("modes");
                let content = std::fs::read_to_string(path).ok()?;
                let first_mode = content.lines().next()?;
                let mut parts = first_mode.split('x');
                let w: f32 = parts.next()?.parse().ok()?;
                Some(w)
            })
        })
        .unwrap_or(1920.0);

    let window_width = 360.0;
    let target_x = screen_width - window_width;
    let target_y = 60.0;

    app.window().set_position(slint::LogicalPosition::new(target_x, target_y));
    info!(target_x, target_y, "Docked window to top-right screen corner ヽ(>∀<☆)ノ");

    app.run()?;
    Ok(())
}