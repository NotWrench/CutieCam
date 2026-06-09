use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct AdbDevice {
    pub id: String,
    pub state: String,
}

pub async fn get_devices() -> Result<Vec<AdbDevice>> {
    let output = Command::new("adb")
        .arg("devices")
        .stdout(Stdio::piped())
        .output()
        .await
        .context("Failed to run adb command. Is ADB installed and in your PATH?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            devices.push(AdbDevice {
                id: parts[0].to_string(),
                state: parts[1].to_string(),
            });
        }
    }
    Ok(devices)
}

pub async fn forward_port(device_id: &str, local_port: u16, remote_port: u16) -> Result<()> {
    let status = Command::new("adb")
        .args([
            "-s",
            device_id,
            "forward",
            &format!("tcp:{}", local_port),
            &format!("tcp:{}", remote_port),
        ])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("Failed to setup ADB port forwarding");
    }
    Ok(())
}

pub async fn remove_forward(device_id: &str, local_port: u16) -> Result<()> {
    let _ = Command::new("adb")
        .args([
            "-s",
            device_id,
            "forward",
            "--remove",
            &format!("tcp:{}", local_port),
        ])
        .status()
        .await;
    Ok(())
}
