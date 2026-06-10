use bytes::Bytes;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use std::time::Duration;

pub async fn run_video_receiver(local_port: u16, tx: broadcast::Sender<Bytes>) {
    loop {
        match TcpStream::connect(format!("127.0.0.1:{}", local_port)).await {
            Ok(mut stream) => {
                println!("Connected to Android video stream via ADB");

                loop {
                    let mut len_buf = [0u8; 4];

                    if stream.read_exact(&mut len_buf).await.is_err() {
                        println!("Android disconnected. Reconnecting in 2 seconds...");
                        break;
                    }

                    let frame_len = u32::from_be_bytes(len_buf) as usize;

                    if frame_len > 10 * 1024 * 1024 {
                        eprintln!("Frame too large ({} bytes). Dropping connection to reset state.", frame_len);
                        break;
                    }

                    let mut frame_buf = vec![0u8; frame_len];

                    if stream.read_exact(&mut frame_buf).await.is_err() {
                        println!("Android disconnected during frame read. Reconnecting in 2 seconds...");
                        break;
                    }

                    let _ = tx.send(Bytes::from(frame_buf));
                }

                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}