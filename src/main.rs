slint::include_modules!();

use std::error::Error;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Force X11/XWayland backend so KDE KWin allows custom window positioning
    if std::env::var("WINIT_UNIX_BACKEND").is_err() {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to initialize tracing logger");

    info!("NetCassette KDE docking engine starting... (ง •̀_•́)ง");

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
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        if let Some(ui) = dock_handle.upgrade() {
            let target_x = screen_width - 360.0;
            ui.window().set_position(slint::LogicalPosition::new(target_x, 0.0));
            info!(target_x, "KDE KWin window pinned flush to top-right corner (✧ω✧)");
        }
    });

    // Show initially for 2.5s, then hide into edge
    let timer_handle = app_handle.clone();
    slint::Timer::single_shot(std::time::Duration::from_millis(2500), move || {
        if let Some(ui) = timer_handle.upgrade() {
            ui.set_initial_reveal(false);
            info!("Initial welcome reveal ended: tucked into right edge (˘ω˘)");
        }
    });

    app.run()?;
    Ok(())
}