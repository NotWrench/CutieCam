use anyhow::{Context, Result};
use bytes::Bytes;
use image::load_from_memory;
use std::sync::Arc;
use tokio::sync::broadcast;
use virtualcam::{Camera, PixelFormat};

pub struct AppState {
    pub frame_rx: broadcast::Sender<Bytes>,
}

pub async fn run_virtual_camera(state: Arc<AppState>) -> Result<()> {
    let mut rx = state.frame_rx.subscribe();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut cam: Option<Camera> = None;
        let mut current_width = 0;
        let mut current_height = 0;

        loop {
            // Explicitly handle channel lag instead of exiting the loop
            let frame_bytes = match rx.blocking_recv() {
                Ok(b) => b,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // CPU couldn't decode fast enough, so we dropped some frames.
                    // Just skip and continue to the newest frame!
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // The video receiver task died, so we should exit.
                    break;
                }
            };

            let img = match load_from_memory(&frame_bytes) {
                Ok(img) => img.into_rgb8(),
                Err(e) => {
                    eprintln!("⚠️ Failed to decode JPEG: {}", e);
                    continue;
                }
            };

            let width = img.width();
            let height = img.height();

            if cam.is_none() || current_width != width || current_height != height {
                println!("📸 Initializing Windows Virtual Camera at {}x{}", width, height);

                let new_cam = Camera::builder(width, height, 30.0)
                    .format(PixelFormat::RGB)
                    .build()
                    .context("❌ Failed to bind to OS Virtual Camera. Make sure the OBS Studio driver is installed on Windows!")?;

                println!("✅ Virtual Camera active! Device connected: {}", new_cam.device());

                cam = Some(new_cam);
                current_width = width;
                current_height = height;
            }

            if let Some(c) = cam.as_mut() {
                if let Err(e) = c.send(img.as_raw()) {
                    eprintln!("⚠️ Failed to send frame to OS driver: {}", e);
                }
            }
        }
        Ok(())
    })
        .await??;

    Ok(())
}