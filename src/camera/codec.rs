use std::io;

use bytes::{Bytes, BytesMut};

use memchr::memmem;
use smol_str::SmolStr;
use tokio_util::codec::{Decoder, Encoder};

/// JPEG start.
const JPEG_START_MARKER: [u8; 4] = [0xff, 0xd8, 0xff, 0xe0];

/// JPEG end.
const JPEG_END_MARKER: [u8; 2] = [0xff, 0xd9];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraPacket {
    Auth {
        username: SmolStr,
        access_code: SmolStr,
    },
    Jpeg(Bytes),
}

/// A simple Tokio Codec that scans for JPEG frames based on start/end markers.
#[derive(Default)]
pub struct JpegCodec(());

fn decode_jpeg_packet(src: &mut BytesMut) -> io::Result<Option<Bytes>> {
    // 1) Look for the start marker
    let start_idx = match find_subsequence(src, &JPEG_START_MARKER) {
        Some(idx) => idx,
        None => return Ok(None), // not found yet
    };

    // 2) Look for the end marker *after* the start
    let search_start = start_idx + JPEG_START_MARKER.len();
    let end_rel_idx = match find_subsequence(&src[search_start..], &JPEG_END_MARKER) {
        Some(idx) => idx,
        None => return Ok(None), // haven't found the complete end yet
    };

    // Actual end is offset from search_start
    let end_idx = search_start + end_rel_idx + JPEG_END_MARKER.len();

    // 3) Remove that bytes region from `src`
    let mut head = src.split_to(end_idx); // remove everything up to end_idx

    // 4) We now have the full [start_idx .. end_idx] inclusive
    let frame = head.split_off(start_idx);

    // 5) Return the frame
    Ok(Some(frame.freeze()))
}

fn decode_auth_packet(src: &mut BytesMut) -> io::Result<Option<(SmolStr, SmolStr)>> {
    if src.len() >= 80 {
        // Parse the 4 x u32 fields
        let magic_1 = u32::from_le_bytes(src[..4].try_into().unwrap());
        let magic_2 = u32::from_le_bytes(src[4..8].try_into().unwrap());

        let zero_1 = u32::from_le_bytes(src[8..12].try_into().unwrap());
        let zero_2 = u32::from_le_bytes(src[12..16].try_into().unwrap());

        if magic_1 == 64 && magic_2 == 12288 && zero_1 == 0 && zero_2 == 0 {
            let username_part = &src[16..48];
            let access_part = &src[48..80];

            // Trim zeros from each
            let username =
                SmolStr::new(String::from_utf8_lossy(username_part).trim_end_matches('\0'));
            let access_code =
                SmolStr::new(String::from_utf8_lossy(access_part).trim_end_matches('\0'));

            let _raw = src.split_to(80);

            return Ok(Some((username, access_code)));
        }
    }
    Ok(None)
}
/// Helper function to find the first occurrence of a `needle` in `haystack`.
#[inline]
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    memmem::find(haystack, needle)
}

impl Decoder for JpegCodec {
    type Item = CameraPacket;
    type Error = io::Error;

    /// Attempt to decode one complete JPEG frame from `src`.
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // We need 80 bytes for auth packet
        if let Some((username, access_code)) = decode_auth_packet(src)? {
            Ok(Some(CameraPacket::Auth {
                username,
                access_code,
            }))
        } else if let Some(jpeg) = decode_jpeg_packet(src)? {
            Ok(Some(CameraPacket::Jpeg(jpeg)))
        } else {
            Ok(None)
        }
    }
}

impl Encoder<CameraPacket> for JpegCodec {
    type Error = io::Error;

    /// Not used for sending frames in this example, so do nothing.
    fn encode(&mut self, item: CameraPacket, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            CameraPacket::Auth {
                username,
                access_code,
            } => {
                // Auth packet: 0x40, 0x3000, 0, 0 + username + access_code (padded)
                dst.extend_from_slice(&0x40u32.to_le_bytes()); // '@'
                dst.extend_from_slice(&0x3000u32.to_le_bytes()); // '0' with some offset
                dst.extend_from_slice(&0u32.to_le_bytes());
                dst.extend_from_slice(&0u32.to_le_bytes());

                // Write username (up to 32 bytes, padded with zeros)
                let mut username_bytes = [0u8; 32];
                let username_utf8 = username.as_bytes();
                let len = username_utf8.len().min(32);
                username_bytes[..len].copy_from_slice(&username_utf8[..len]);
                dst.extend_from_slice(&username_bytes);

                // Write access_code (up to 32 bytes, padded with zeros)
                let mut access_bytes = [0u8; 32];
                let code_utf8 = access_code.as_bytes();
                let len = code_utf8.len().min(32);
                access_bytes[..len].copy_from_slice(&code_utf8[..len]);
                dst.extend_from_slice(&access_bytes);
            }
            CameraPacket::Jpeg(bytes) => dst.extend_from_slice(&bytes),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_subsequence() {
        let haystack = b"hello world";
        let needle = b"world";
        assert_eq!(find_subsequence(haystack, needle), Some(6));

        let needle = b"foo";
        assert_eq!(find_subsequence(haystack, needle), None);
    }

    #[test]
    fn test_decode_complete_frame() {
        let mut codec = JpegCodec::default();
        let mut src = BytesMut::from(&b"\xff\xd8\xff\xe0hello world\xff\xd9"[..]);
        let expected = src.clone().freeze();

        let frame = codec.decode(&mut src).unwrap().unwrap();
        assert_eq!(frame, CameraPacket::Jpeg(expected));
        assert!(src.is_empty());
    }

    #[test]
    fn test_decode_partial_frame() {
        let mut codec = JpegCodec::default();
        let mut src = BytesMut::from(&b"\xff\xd8\xff\xe0hello"[..]);

        let frame = codec.decode(&mut src).unwrap();
        assert!(frame.is_none());
        assert_eq!(src, &b"\xff\xd8\xff\xe0hello"[..]);
    }

    #[test]
    fn test_decode_multiple_frames() {
        let mut codec = JpegCodec::default();
        let mut src =
            BytesMut::from(&b"\xff\xd8\xff\xe0frame1\xff\xd9\xff\xd8\xff\xe0frame2\xff\xd9"[..]);

        let frame1 = codec.decode(&mut src).unwrap().unwrap();
        assert_eq!(
            frame1,
            CameraPacket::Jpeg(Bytes::from_static(b"\xff\xd8\xff\xe0frame1\xff\xd9"))
        );

        let frame2 = codec.decode(&mut src).unwrap().unwrap();
        assert_eq!(
            frame2,
            CameraPacket::Jpeg(Bytes::from_static(b"\xff\xd8\xff\xe0frame2\xff\xd9"))
        );

        assert!(src.is_empty());
    }

    #[test]
    fn test_decode_no_start_marker() {
        let mut codec = JpegCodec::default();
        let mut src = BytesMut::from(&b"hello world\xff\xd9"[..]);

        let frame = codec.decode(&mut src).unwrap();
        assert!(frame.is_none());
        assert_eq!(src, &b"hello world\xff\xd9"[..]);
    }

    #[test]
    fn test_decode_no_end_marker() {
        let mut codec = JpegCodec::default();
        let mut src = BytesMut::from(&b"\xff\xd8\xff\xe0hello world"[..]);

        let frame = codec.decode(&mut src).unwrap();
        assert!(frame.is_none());
        assert_eq!(src, &b"\xff\xd8\xff\xe0hello world"[..]);
    }

    #[test]
    fn test_decode_invalid_marker_regression() {
        let mut codec = JpegCodec::default();
        let mut src = BytesMut::from(&b"\xff\xd8\xff\xe0\0!AVI1\0\x01\x01\x01\0x\0x\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\xff\xdb"[..]);

        let frame = codec.decode(&mut src).unwrap();
        assert!(frame.is_none());
        assert_eq!(src, &b"\xff\xd8\xff\xe0\0!AVI1\0\x01\x01\x01\0x\0x\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\xff\xdb"[..]);
    }

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

    #[test]
    fn foo() {
        let mut stream = create_auth_packet("bblp", "1234");
        let image1 = {
            let mut image = Vec::new();
            image.extend_from_slice(&JPEG_START_MARKER);
            image.extend_from_slice(b"foobar");
            image.extend_from_slice(&JPEG_END_MARKER);
            image
        };

        stream.extend_from_slice(&image1);
        let mut codec = JpegCodec::default();
        let mut src = BytesMut::from(dbg!(Bytes::from(stream)));
        assert_eq!(
            codec.decode(&mut src).unwrap(),
            Some(CameraPacket::Auth {
                username: "bblp".into(),
                access_code: "1234".into(),
            })
        );
        assert_eq!(
            codec.decode(&mut src).unwrap(),
            Some(CameraPacket::Jpeg(Bytes::from(image1)))
        );
    }
}
