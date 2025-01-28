use bytes::{Buf, BufMut, BytesMut};
use memchr::memchr;
use memchr::memmem;
use smallvec::SmallVec;
use smol_str::format_smolstr;
use smol_str::SmolStr;
use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
use tokio_util::codec::{Decoder, Encoder};

const CRLF: &[u8] = b"\r\n";

/// Represents specific FTP commands (requests).
#[derive(Debug)]
pub enum FtpRequest {
    User(String),
    Pass(String),
    Quit,
    EnterPassiveMode,
    List(String),              // Directory to list files
    ProtectionBufferSize(u32), // Protection Buffer Size
    ProtectionLevel(String),   // Protection Level
    Pwd,                       // Print Working Directory
}

impl FtpRequest {
    /// Converts an `FtpMessage` into a raw string command.
    fn to_command_string(&self) -> SmolStr {
        match self {
            FtpRequest::User(username) => format_smolstr!("USER {}", username),
            FtpRequest::Pass(password) => format_smolstr!("PASS {}", password),
            FtpRequest::Quit => SmolStr::new_static("QUIT"),
            FtpRequest::EnterPassiveMode => SmolStr::new_static("PASV"),
            FtpRequest::List(path) => format_smolstr!("LIST {}", path),
            FtpRequest::ProtectionBufferSize(size) => format_smolstr!("PBSZ {}", size),
            FtpRequest::ProtectionLevel(level) => format_smolstr!("PROT {}", level),
            FtpRequest::Pwd => SmolStr::new_static("PWD"),
        }
    }
}
/// Represents FTP server responses.
#[derive(Debug)]
pub enum FtpResponse {
    FileStatusOkay(String),           // 150
    ServiceReady(String),             // 220
    CommandOkay(String),              // 200
    ClosingControlConnection(String), // 221
    ClosingDataConnection(String),    // 226
    UserLoggedIn(String),             // 230
    UserNameOkayNeedPassword(String), // 331
    #[allow(dead_code)]
    FileActionOkay(String), // 250
    EnteringPassiveMode(SocketAddr),  // 227
    #[allow(dead_code)]
    CommandNotImplemented(String), // 502
    #[allow(dead_code)]
    BadSequenceOfCommands(String), // 503
    #[allow(dead_code)]
    FileUnavailable(String), // 550
    #[allow(dead_code)]
    DirectoryActionOkay(String), // 257
    #[allow(dead_code)]
    Other(u16, String), // For unhandled or unknown responses
}

impl FtpResponse {
    /// Parses a raw response string into an `FtpResponse`.
    pub fn from_response_string(response: &str) -> Result<Self, std::io::Error> {
        let mut parts = response.splitn(2, ' ');
        let code = parts
            .next()
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid response code")
            })?;
        let message = parts.next().unwrap_or("").to_string();

        match code {
            150 => Ok(FtpResponse::FileStatusOkay(message)),
            220 => Ok(FtpResponse::ServiceReady(message)),
            200 => Ok(FtpResponse::CommandOkay(message)),
            226 => Ok(FtpResponse::ClosingDataConnection(message)),
            230 => Ok(FtpResponse::UserLoggedIn(message)),
            331 => Ok(FtpResponse::UserNameOkayNeedPassword(message)),
            250 => Ok(FtpResponse::FileActionOkay(message)),
            227 => {
                // Find '(' and ')' via memchr and validate there's exactly one pair in the right order.
                let bytes = message.as_bytes();
                let start = memchr(b'(', bytes).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "Missing '(' in PASV response")
                })?;
                let end_rel = memchr(b')', &bytes[start..]).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "Missing ')' in PASV response")
                })?;
                let end = start + end_rel;

                // Disallow any extra parentheses or reversed order.
                if end == start
                    || memchr(b'(', &bytes[start + 1..end]).is_some()
                    || memchr(b')', &bytes[start + 1..end]).is_some()
                    || memchr(b'(', &bytes[end + 1..]).is_some()
                    || memchr(b')', &bytes[..start]).is_some()
                    || memchr(b')', &bytes[end + 1..]).is_some()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid PASV response",
                    ));
                }

                // Extract IP and port data.
                let data = &message[start + 1..end];
                let parts: SmallVec<[&str; 6]> = data.split(',').collect();
                if parts.len() != 6 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid PASV response",
                    ));
                }

                // Parse the IP and port
                let ip_address: Ipv4Addr = {
                    let a = parts[0]
                        .parse::<u8>()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    let b = parts[1]
                        .parse::<u8>()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    let c = parts[2]
                        .parse::<u8>()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    let d = parts[3]
                        .parse::<u8>()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    Ipv4Addr::new(a, b, c, d)
                };

                let port_hi = parts[4]
                    .parse::<u16>()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                let port_lo = parts[5]
                    .parse::<u16>()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                let port = port_hi * 256 + port_lo;

                let socket_address = SocketAddr::new(IpAddr::V4(ip_address), port);

                Ok(FtpResponse::EnteringPassiveMode(socket_address))
            }
            221 => Ok(FtpResponse::ClosingControlConnection(message)),
            502 => Ok(FtpResponse::CommandNotImplemented(message)),
            503 => Ok(FtpResponse::BadSequenceOfCommands(message)),
            550 => Ok(FtpResponse::FileUnavailable(message)),
            257 => Ok(FtpResponse::DirectoryActionOkay(message)),
            _ => Ok(FtpResponse::Other(code, message)),
        }
    }
}

/// Codec for encoding and decoding FTP commands and responses.
pub struct FtpCodec;

impl Encoder<FtpRequest> for FtpCodec {
    type Error = io::Error;

    fn encode(&mut self, item: FtpRequest, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let command = item.to_command_string();
        dst.reserve(command.len() + CRLF.len());
        dst.put(command.as_bytes());
        dst.put(CRLF); // Append CRLF
        Ok(())
    }
}

impl Decoder for FtpCodec {
    type Item = FtpResponse;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(pos) = memmem::find(src, CRLF) {
            // Extract the line up to CRLF
            let line = src.split_to(pos);
            src.advance(2); // Consume CRLF

            let line = std::str::from_utf8(&line)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            // Parse into an FtpResponse
            let response = FtpResponse::from_response_string(line)?;
            return Ok(Some(response));
        }
        Ok(None) // Wait for more data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pasv_response() {
        let response = "227 Entering Passive Mode (192,168,1,2,4,3)";
        let response = FtpResponse::from_response_string(response).unwrap();
        match response {
            FtpResponse::EnteringPassiveMode(addr) => {
                assert_eq!(
                    addr,
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 1027)
                );
            }
            _ => panic!("Expected EnteringPassiveMode"),
        }
    }

    #[test]
    fn test_invalid_pasv_response() {
        let response = "227 Entering Passive Mode (192,168,1,2,4,3)";
        let response = FtpResponse::from_response_string(response).unwrap();
        match response {
            FtpResponse::EnteringPassiveMode(addr) => {
                assert_eq!(
                    addr,
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 1027)
                );
            }
            _ => panic!("Expected EnteringPassiveMode"),
        }
    }
}
