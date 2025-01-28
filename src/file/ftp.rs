mod codec;
pub mod metadata;

use codec::{FtpCodec, FtpRequest, FtpResponse};
use futures_util::{SinkExt, StreamExt};
use metadata::FileMetadata;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types;
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;
use tokio_util::codec::{Framed, LinesCodec};

use super::NoVerifier;

const FTPS_PORT: u16 = 990;

pub struct FtpClient {
    hostname: String,
    username: String,
    password: String,
    framed: Framed<TlsStream<TcpStream>, FtpCodec>,
}

impl FtpClient {
    pub async fn connect(hostname: String, username: String, password: String) -> io::Result<Self> {
        let port = FTPS_PORT;
        // TCP connection

        let socket_addr = (hostname.as_str(), port).to_socket_addrs()?.next().unwrap();

        let framed = connect_insecure(socket_addr, FtpCodec).await?;

        Ok(Self {
            hostname,
            username,
            password,
            framed,
        })
    }

    /// Sends a command to the FTP server and reads the response.
    async fn send_command(&mut self, command: FtpRequest) -> io::Result<FtpResponse> {
        self.framed.send(command).await?;
        if let Some(response) = self.framed.next().await.transpose()? {
            // Fix some responses to only pass valid data to the caller.
            let response = match response {
                FtpResponse::EnteringPassiveMode(socket_addr) => {
                    if socket_addr.ip().is_unspecified() {
                        FtpResponse::EnteringPassiveMode(SocketAddr::new(
                            self.hostname.parse().unwrap(), // NOTE: Technically, this is validated while connecting.
                            socket_addr.port(),
                        ))
                    } else {
                        FtpResponse::EnteringPassiveMode(socket_addr)
                    }
                }
                other => other,
            };

            Ok(response)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid response",
            ))
        }
    }

    pub async fn authenticate(&mut self) -> io::Result<Option<String>> {
        // Read server's welcome message
        let message = if let Some(FtpResponse::ServiceReady(message)) =
            self.framed.next().await.transpose()?
        {
            println!("FTP server: {}", message);
            Some(message)
        } else {
            None
        };

        // Authenticate
        let user_response = self
            .send_command(FtpRequest::User(self.username.clone()))
            .await
            .unwrap();

        match user_response {
            FtpResponse::UserNameOkayNeedPassword(message) => {
                if !message.is_empty() {
                    println!("Username okay, need password: {}", message);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid username response",
                ));
            }
        }

        let password_response = self
            .send_command(FtpRequest::Pass(self.password.clone()))
            .await
            .unwrap();

        match password_response {
            FtpResponse::UserLoggedIn(message) => {
                if !message.is_empty() {
                    println!("User logged in: {}", message);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid password response",
                ));
            }
        }

        // Control messages
        let response = self
            .send_command(FtpRequest::ProtectionBufferSize(0))
            .await?;
        match response {
            FtpResponse::CommandOkay(message) => {
                if !message.is_empty() {
                    println!("Protection buffer size okay: {}", message);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid PBSZ response",
                ));
            }
        }

        let response = self
            .send_command(FtpRequest::ProtectionLevel("P".to_string()))
            .await?;
        match response {
            FtpResponse::CommandOkay(message) => {
                if !message.is_empty() {
                    println!("Protection level okay: {}", message);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid PROT response",
                ));
            }
        }

        Ok(message)
    }

    pub async fn pwd(&mut self) -> io::Result<String> {
        let response = self.send_command(FtpRequest::Pwd).await?;
        match response {
            FtpResponse::DirectoryActionOkay(message) => Ok(message),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid PWD response",
            )),
        }
    }

    pub async fn quit(&mut self) -> io::Result<()> {
        let response = self.send_command(FtpRequest::Quit).await?;
        match response {
            FtpResponse::ClosingControlConnection(message) => {
                if !message.is_empty() {
                    println!("Closing control connection: {}", message);
                }
            }
            FtpResponse::ClosingDataConnection(message) => {
                if !message.is_empty() {
                    println!("Closing control connection: {}", message);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid QUIT response",
                ));
            }
        }
        Ok(())
    }

    /// Connects to the FTP server and lists files in the given directory.
    pub async fn list_files(&mut self, directory: &str) -> io::Result<Vec<FileMetadata>> {
        let pwd = self.pwd().await?;
        println!("Current directory: {}", pwd);

        // Enter passive mode
        let pasv_response = self.send_command(FtpRequest::EnterPassiveMode).await?;

        let socket_addr = match pasv_response {
            FtpResponse::EnteringPassiveMode(socket_addr) => socket_addr,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid passive mode response",
                ));
            }
        };

        let response = self
            .send_command(FtpRequest::List(directory.to_string()))
            .await?;
        match response {
            FtpResponse::FileStatusOkay(message) => {
                if !message.is_empty() {
                    println!("File status okay: {}", message);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid LIST response",
                ));
            }
        }

        // Connect to the data stream
        let lines = {
            let mut data_framed = connect_insecure(socket_addr, LinesCodec::new()).await?;
            println!("Connected to {:?}", socket_addr);

            let mut lines = Vec::new();
            while let Some(response) = data_framed.next().await {
                match response {
                    Ok(line) => {
                        let file_metadata = FileMetadata::from_str(&line)
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                        lines.push(file_metadata);

                        // lines.push(FileMetadata::from_str(&line);
                    }
                    Err(e) => {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, e));
                    }
                }
            }
            lines
        };

        self.quit().await?;

        Ok(lines)
    }
}

async fn connect_insecure<C>(
    address: SocketAddr,
    codec: C,
) -> Result<Framed<TlsStream<TcpStream>, C>, io::Error> {
    let tcp_stream = TcpStream::connect(address).await?;
    // tcp_stream.

    let config: ClientConfig = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier))
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));

    let tls_stream = connector
        .connect(
            pki_types::ServerName::IpAddress(pki_types::IpAddr::from(address.ip())),
            tcp_stream,
        )
        .await?;
    let framed = Framed::new(tls_stream, codec);
    Ok(framed)
}
