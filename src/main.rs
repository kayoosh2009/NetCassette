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

    app.on_minimize_clicked({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                ui.window().set_minimized(true);
                info!("Window minimized ( ˘ω˘ )");
            }
        }
    });

    app.on_close_clicked({
        move || {
            info!("Shutting down NetCassette... ( ﾟヮﾟ)ﾉ");
            std::process::exit(0);
        }
    });

    app.run()?;
    Ok(())
}