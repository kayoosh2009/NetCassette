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

    info!("NetCassette edge-dock widget launching... (｡•̀ᴗ-)✧");

    let app = MainWindow::new()?;
    let app_handle = app.as_weak();

    // Toggle playback action
    app.on_toggle_playback({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                let next = !ui.get_is_playing();
                ui.set_is_playing(next);
                ui.set_stream_status(if next { "Live".into() } else { "Paused".into() });
                info!(playback = next, "Playback state changed");
            }
        }
    });

    // Swarm join action
    app.on_connect_peer({
        let handle = app_handle.clone();
        move |room: slint::SharedString| {
            let room_id = room.to_string();
            if room_id.trim().is_empty() {
                return;
            }

            info!(swarm_room = %room_id, "Joining tape swarm");
            if let Some(ui) = handle.upgrade() {
                ui.set_stream_status("Syncing...".into());
                ui.set_track_title(format!("Cassette #{}", &room_id[..room_id.len().min(8)]).into());
                ui.set_peer_count(1);
            }
        }
    });

    // Close button action
    app.on_close_widget({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                let _ = ui.hide();
                info!("Widget dismissed by user (˘ω˘)");
                std::process::exit(0);
            }
        }
    });

    app.run()?;
    Ok(())
}