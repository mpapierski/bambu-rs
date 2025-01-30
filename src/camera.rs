pub mod codec;

use std::sync::Arc;

use codec::{CameraPacket, JpegCodec};
use futures_util::SinkExt;
use smol_str::SmolStr;
use tokio::net::TcpStream;
use tokio_rustls::{
    client::TlsStream,
    rustls::{
        pki_types::{IpAddr, ServerName},
        ClientConfig,
    },
    TlsConnector,
};
use tokio_util::codec::Framed;

use crate::tls::NoVerifier;

const DEFAULT_CAMERA_USERNAME: &str = "bblp";

/// Asynchronous camera client.
pub struct CameraClient {
    hostname: String,
    access_code: String,
    port: u16,
}

impl CameraClient {
    /// Create a new `CameraClient`.
    pub fn new(hostname: &str, access_code: &str, port: u16) -> Self {
        Self {
            hostname: hostname.to_string(),
            access_code: access_code.to_string(),
            port,
        }
    }

    /// Connect via TCP + TLS, send the auth packet, and then return a `Framed`
    /// that uses `JpegCodec` to decode JPEG frames from the socket.
    pub async fn connect_and_stream_codec(
        &self,
    ) -> Result<Framed<TlsStream<TcpStream>, JpegCodec>, Box<dyn std::error::Error>> {
        // 1) Connect via TCP
        let addr = format!("{}:{}", self.hostname, self.port);
        let tcp_stream = TcpStream::connect(&addr).await?;

        // CryptoProvider::install();

        // 2) Create a rustls ClientConfig
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        let config = Arc::new(config);
        let connector = TlsConnector::from(config);

        // 3) Wrap in tokio-rustls for async TLS
        let ip_address = IpAddr::try_from(self.hostname.as_str()).unwrap();
        let tls_stream = connector
            .connect(ServerName::IpAddress(ip_address), tcp_stream)
            .await?;

        // 4) Wrap with Framed + JpegCodec
        let mut framed = Framed::new(tls_stream, JpegCodec::default());

        // 5) Send auth data first
        framed
            .send(CameraPacket::Auth {
                username: DEFAULT_CAMERA_USERNAME.into(),
                access_code: SmolStr::from(self.access_code.clone()),
            })
            .await?;
        // Flush to ensure the server receives it
        framed.flush().await?;

        Ok(framed)
    }
}
