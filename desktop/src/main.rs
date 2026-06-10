mod adb;
mod video;
mod virtual_cam;

use anyhow::Context;
use std::sync::Arc;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let devices = adb::get_devices().await?;
    if devices.is_empty() {
        anyhow::bail!("No Android devices found. Please connect a device and enable USB Debugging.");
    }

    println!("Connected devices:");
    for (i, dev) in devices.iter().enumerate() {
        println!("  [{}] {} ({})", i, dev.id, dev.state);
    }

    let device = if devices.len() == 1 {
        &devices[0]
    } else {
        use std::io::Write;
        print!("Select device index: ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let index: usize = input.trim().parse().context("Invalid input")?;
        devices.get(index).context("Invalid device index")?
    };

    if device.state != "device" {
        anyhow::bail!("Device is unauthorized. Please check your phone screen and accept the ADB connection prompt.");
    }

    println!("\nSelected device: {}", device.id);

    let local_port = 7879;
    let remote_port = 8080;
    println!("Forwarding Local TCP:{} -> Android TCP:{}", local_port, remote_port);
    adb::forward_port(&device.id, local_port, remote_port).await?;

    let (tx, _rx) = broadcast::channel(16);
    let app_state = Arc::new(virtual_cam::AppState {
        frame_rx: tx.clone(),
    });

    tokio::spawn(async move {
        video::run_video_receiver(local_port, tx).await;
    });

    tokio::select! {
        res = virtual_cam::run_virtual_camera(app_state) => {
            if let Err(e) = res {
                eprintln!("Virtual Camera error: {}", e);
            }
        },
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down gracefully...");
        }
    }

    println!("Cleaning up ADB port forwards...");
    adb::remove_forward(&device.id, local_port).await?;

    Ok(())
}