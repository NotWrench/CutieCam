use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

pub struct AppState {
    pub frame_rx: broadcast::Sender<Bytes>,
}

pub async fn run_server(port: u16, state: Arc<AppState>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/video", get(video_handler))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("OBS Stream running at: http://{}/video", addr);
    println!("Add a 'Media Source' in OBS and uncheck 'Local File', then paste the URL above.");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn video_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rx = state.frame_rx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(frame) => {
            let mut chunk = Vec::new();
            chunk.extend_from_slice(b"--cutiecam_boundary\r\n");
            chunk.extend_from_slice(b"Content-Type: image/jpeg\r\n");
            chunk.extend_from_slice(format!("Content-Length: {}\r\n\r\n", frame.len()).as_bytes());
            chunk.extend_from_slice(&frame);
            chunk.extend_from_slice(b"\r\n");

            Some(Ok::<_, anyhow::Error>(Bytes::from(chunk)))
        }
        Err(_) => None,
    });

    let body = Body::from_stream(stream);

    let headers = [
        (
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=cutiecam_boundary",
        ),
        (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
        (header::PRAGMA, "no-cache"),
        (header::EXPIRES, "0"),
    ];

    (StatusCode::OK, headers, body)
}
