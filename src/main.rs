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

    info!("Starting NetCassette P2P audio node...");

    let app = MainWindow::new()?;
    let app_handle = app.as_weak();

    // Toggle playback event handler
    app.on_toggle_playback({
        let handle = app_handle.clone();
        move || {
            if let Some(ui) = handle.upgrade() {
                let current_state = ui.get_is_playing();
                let next_state = !current_state;
                ui.set_is_playing(next_state);
                
                if next_state {
                    ui.set_stream_status("Buffering swarm...".into());
                    info!("Playback requested: starting stream pipelines");
                } else {
                    ui.set_stream_status("Paused".into());
                    info!("Playback paused");
                }
            }
        }
    });

    // P2P swarm connect event handler
    app.on_connect_peer({
        let handle = app_handle.clone();
        move |target: slint::SharedString| {
            let target_str = target.to_string();
            if target_str.trim().is_empty() {
                return;
            }

            info!(target = %target_str, "Connection command received from UI");
            
            if let Some(ui) = handle.upgrade() {
                ui.set_stream_status("Connecting to peer swarm...".into());
                // Dummy feedback before real P2P driver is wired
                ui.set_peer_count(ui.get_peer_count() + 1);
                ui.set_track_title(format!("Tape Stream [{}]", &target_str[..target_str.len().min(12)]).into());
            }
        }
    });

    app.run()?;
    info!("NetCassette shutdown gracefully (─‿‿─)");

    Ok(())
}