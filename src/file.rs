//! A module for interacting with BambuLab file server.
pub(crate) mod ftp;

use std::io;

use crate::tls::NoVerifier;
use ftp::{metadata::FileMetadata, FtpClient};

/// An async FTPS file client, similar to the Python version using curl.
/// It can list files in a directory and download specific files.
pub struct FileClient {
    hostname: String,
    access_code: String,
}

impl FileClient {
    /// Create a new FileClient.
    ///
    /// * `hostname`: The FTP(S) server hostname.
    /// * `access_code`: The password to use with user "bblp".
    /// * `serial`: Some identifier (unused in this snippet, but kept for parity).
    /// * `insecure`: If `true`, client will skip certificate validation.
    pub fn new(hostname: impl Into<String>, access_code: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
            access_code: access_code.into(),
        }
    }

    /// List files in the given `directory`, filtering by `extension`.
    /// This is roughly equivalent to running:
    /// `curl --ftp-pasv --insecure ftps://HOSTNAME/DIRECTORY --user bblp:ACCESS_CODE`.
    pub async fn get_files(&self, directory: &str) -> io::Result<Vec<FileMetadata>> {
        // Connect to the server
        // let mut ftp_stream = self.connect_and_login().await?;
        let mut client = FtpClient::connect(
            self.hostname.clone(),
            "bblp".to_string(),
            self.access_code.clone(),
        )
        .await
        .unwrap();
        let _message = client.authenticate().await?;
        let files = client.list_files(directory).await?;
        client.quit().await?;
        Ok(files)
    }
}
