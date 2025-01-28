use async_stream::try_stream;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, Response},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use bambu::CameraClient;
use futures_core::Stream;
use futures_util::StreamExt;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::{
    sync::{
        broadcast::{self, Receiver},
        RwLock,
    },
    task,
};
use turbojpeg::DecompressHeader;

async fn read_jpeg_header(jpeg_frame_bytes: Bytes) -> turbojpeg::Result<DecompressHeader> {
    task::spawn_blocking(move || turbojpeg::read_header(&jpeg_frame_bytes))
        .await
        .unwrap()
}

pub(crate) struct Config {
    pub(crate) printer_ip: String,
    pub(crate) access_code: String,
    #[allow(dead_code)]
    pub(crate) serial_number: String,
    pub(crate) camera_port: u16,
}

impl Config {
    pub(crate) fn from_env() -> Self {
        let printer_ip = std::env::var("BAMBU_IP").expect("BAMBU_IP must be set");
        let access_code =
            std::env::var("BAMBU_ACCESS_CODE").expect("BAMBU_ACCESS_CODE must be set");
        let serial_number =
            std::env::var("BAMBU_SERIAL_NUMBER").expect("BAMBU_SERIAL_NUMBER must be set");
        let camera_port: u16 = std::env::var("BAMBU_PORT")
            .unwrap_or_else(|_| "6000".to_string())
            .parse()
            .expect("BAMBU_PORT must be a valid u16");

        Self {
            printer_ip,
            access_code,
            serial_number,
            camera_port,
        }
    }
}

const BOUNDARY: &str = "donotcrossboundary";

#[derive(Clone)]
struct AppState {
    /// We'll use a broadcast channel to send frames to all connections
    tx: broadcast::Sender<Bytes>,
    /// The last frame received from the camera.
    last_frame: Arc<RwLock<Option<Bytes>>>,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a broadcast channel with buffer size = 16 frames
    let (tx, _rx) = broadcast::channel(16);

    let config = Arc::new(Config::from_env());

    let last_frame = Arc::new(RwLock::new(None));

    // Spawn task to connect to the camera and send frames to the broadcast channel.
    tokio::spawn({
        let tx = tx.clone();
        let last_frame = last_frame.clone();
        let config = config.clone();
        async move {
            let client =
                CameraClient::new(&config.printer_ip, &config.access_code, config.camera_port);

            let mut frame_stream = match client.connect_and_stream_codec().await {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("Error connecting to camera: {}", e);
                    return;
                }
            };

            // Consume frames in a loop
            while let Some(jpeg_frame_bytes) = frame_stream.next().await {
                match jpeg_frame_bytes {
                    Ok(jpeg_frame_bytes) => {
                        println!("Received a JPEG frame of length {}", jpeg_frame_bytes.len());

                        // Decode image
                        let jpeg_header = match read_jpeg_header(jpeg_frame_bytes.clone()).await {
                            Ok(img) => img,
                            Err(e) => {
                                eprintln!("Error decoding image: {}", e);
                                continue;
                            }
                        };

                        println!(
                            "Image dimensions: {}x{}",
                            jpeg_header.width, jpeg_header.height
                        );

                        {
                            // Store the last frame in the shared state
                            let mut last_frame = last_frame.write().await;
                            *last_frame = Some(jpeg_frame_bytes.clone());
                        }

                        if tx.send(jpeg_frame_bytes).is_err() {
                            eprintln!("Error sending frame to broadcast channel");
                            break;
                        }
                    }
                    Err(e) => eprintln!("Error receiving frame: {}", e),
                }
            }
        }
    });

    let app_state = Arc::new(AppState { tx, last_frame });
    let app = Router::new()
        .route("/stream/live.mjpeg", get(mjpeg_live_stream))
        .route(
            "/",
            get(|| async { Html(r#"<img src="/stream/live.mjpeg"/>"#) }),
        )
        .with_state(app_state);

    // Start the Axum server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Serving content on http://{}", addr);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

fn mjpeg_stream(
    state: Arc<AppState>,
    mut rx: Receiver<Bytes>,
) -> impl Stream<Item = Result<Bytes, Infallible>> {
    // Build a streaming body using async-stream

    try_stream! {
        // Send the last frame first if available

        if let Some(frame) = state.last_frame.read().await.as_ref() {
            // --frame
            // Content-Type: image/jpeg
            // Content-Length: <len>
            // <JPEG bytes>
            // \r\n

            let header = format!(
                "--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                frame.len()
            );
            yield Bytes::from(header);
            yield frame.clone();
            yield Bytes::from_static(b"\r\n");
        }

        loop {
            let frame_bytes = match rx.recv().await {
                Ok(data) => data,
                Err(_) => {
                    eprintln!("Error receiving frame from broadcast channel");
                     // Sender dropped or other error, end stream
                    break
                },
            };

            let header = format!(
                "--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                frame_bytes.len()
            );

            // Yield boundary + headers
            yield Bytes::from(header);
            // Yield the actual JPEG data
            yield frame_bytes;
            // Yield a trailing newline
            yield Bytes::from_static(b"\r\n");
        }
    }
}

/// Handler that returns an MJPEG stream.
async fn mjpeg_live_stream(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Create a new broadcast receiver for this connection
    let rx = state.tx.subscribe();

    // Create the response with the correct Content-Type header
    Response::builder()
        .header(
            header::CONTENT_TYPE,
            format!("multipart/x-mixed-replace; boundary={BOUNDARY}"),
        )
        .body(Body::from_stream(mjpeg_stream(Arc::clone(&state), rx)))
        .unwrap()
}
