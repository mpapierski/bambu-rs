mod codec;

use std::sync::Arc;

use codec::JpegCodec;
use tokio::{io::AsyncWriteExt, net::TcpStream};
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

/// Build the authentication packet
fn create_auth_packet(username: &str, access_code: &str) -> Vec<u8> {
    let mut auth_data = Vec::new();

    // Auth packet: 0x40, 0x3000, 0, 0 + username + access_code (padded)
    auth_data.extend_from_slice(&0x40u32.to_le_bytes()); // '@'
    auth_data.extend_from_slice(&0x3000u32.to_le_bytes()); // '0' with some offset
    auth_data.extend_from_slice(&0u32.to_le_bytes());
    auth_data.extend_from_slice(&0u32.to_le_bytes());

    // Write username (up to 32 bytes, padded with zeros)
    let mut username_bytes = [0u8; 32];
    let username_utf8 = username.as_bytes();
    let len = username_utf8.len().min(32);
    username_bytes[..len].copy_from_slice(&username_utf8[..len]);
    auth_data.extend_from_slice(&username_bytes);

    // Write access_code (up to 32 bytes, padded with zeros)
    let mut access_bytes = [0u8; 32];
    let code_utf8 = access_code.as_bytes();
    let len = code_utf8.len().min(32);
    access_bytes[..len].copy_from_slice(&code_utf8[..len]);
    auth_data.extend_from_slice(&access_bytes);

    auth_data
}

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

        // 2) Create a rustls ClientConfig
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        let config = Arc::new(config);
        let connector = TlsConnector::from(config);

        // 3) Wrap in tokio-rustls for async TLS
        // let dnsname = DNSNameRef::try_from_ascii_str(&self.hostname)?;
        let ip_address = IpAddr::try_from(self.hostname.as_str()).unwrap();
        let mut tls_stream = connector
            .connect(ServerName::IpAddress(ip_address), tcp_stream)
            .await?;

        // 4) Send auth data first
        let auth_packet = create_auth_packet(DEFAULT_CAMERA_USERNAME, &self.access_code);
        tls_stream.write_all(&auth_packet).await?;

        // Flush to ensure the server receives it
        tls_stream.flush().await?;

        // 5) Wrap with Framed + JpegCodec
        let framed = Framed::new(tls_stream, JpegCodec::default());
        Ok(framed)
    }
}
