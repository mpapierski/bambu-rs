pub mod camera;

use async_stream::try_stream;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, Response},
    response::IntoResponse,
    routing::get,
    Router,
};
use camera::CameraClient;
use futures_core::Stream;
use futures_util::StreamExt;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast::{self, Receiver};

#[derive(Clone)]
struct AppState {
    // We'll use a broadcast channel to send frames to all connections
    tx: broadcast::Sender<Bytes>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a broadcast channel with buffer size = 16 frames
    let (tx, _rx) = broadcast::channel(16);

    let printer_ip = std::env::var("BAMBU_IP").expect("BAMBU_IP must be set");
    let access_code = std::env::var("BAMBU_ACCESS_CODE").expect("BAMBU_ACCESS_CODE must be set");
    let camera_port: u16 = std::env::var("BAMBU_PORT")
        .unwrap_or_else(|_| "6000".to_string())
        .parse()
        .expect("BAMBU_PORT must be a valid u16");

    // Spawn task to connect to the camera and send frames to the broadcast channel.
    tokio::spawn({
        let tx = tx.clone();
        async move {
            let client = CameraClient::new(&printer_ip, &access_code, camera_port);

            let mut frame_stream = match client.connect_and_stream_codec().await {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("Error connecting to camera: {}", e);
                    return;
                }
            };

            // Consume frames in a loop
            while let Some(jpeg_frame) = frame_stream.next().await {
                match jpeg_frame {
                    Ok(jpeg_frame) => {
                        println!("Received a JPEG frame of length {}", jpeg_frame.len());

                        if tx.send(jpeg_frame).is_err() {
                            eprintln!("Error sending frame to broadcast channel");
                            break;
                        }
                    }
                    Err(e) => eprintln!("Error receiving frame: {}", e),
                }
            }
        }
    });

    let app_state = Arc::new(AppState { tx });
    let app = Router::new()
        .route("/live", get(live_stream))
        .with_state(app_state);

    // Start the Axum server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Serving MJPEG on http://{}/live", addr);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

fn mjpeg_stream(mut rx: Receiver<Bytes>) -> impl Stream<Item = Result<Bytes, Infallible>> {
    // Build a streaming body using async-stream
    try_stream! {
        loop {
            let frame = match rx.recv().await {
                Ok(data) => data,
                Err(_) => {
                    eprintln!("Error receiving frame from broadcast channel");
                     // Sender dropped or other error, end stream
                    break
                },
            };

            // --frame
            // Content-Type: image/jpeg
            // Content-Length: <len>
            // <JPEG bytes>
            // \r\n
            let header = format!(
                "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                frame.len()
            );

            // Yield boundary + headers
            yield Bytes::from(header);
            // Yield the actual JPEG data
            yield frame;
            // Yield a trailing newline
            yield Bytes::from("\r\n");
        }
    }
}

/// Handler that returns an MJPEG stream.
async fn live_stream(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Create a new broadcast receiver for this connection
    let rx = state.tx.subscribe();

    // Create the response with the correct Content-Type header
    Response::builder()
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .body(Body::from_stream(mjpeg_stream(rx)))
        .unwrap()
}
