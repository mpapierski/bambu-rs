use std::io;

use bytes::{Bytes, BytesMut};

use memchr::memmem;
use tokio_util::codec::{Decoder, Encoder};

/// JPEG start.
const JPEG_START_MARKER: [u8; 4] = [0xff, 0xd8, 0xff, 0xe0];

/// JPEG end.
const JPEG_END_MARKER: [u8; 2] = [0xff, 0xd9];

/// A simple Tokio Codec that scans for JPEG frames based on start/end markers.
#[derive(Default)]
pub struct JpegCodec(());

/// Helper function to find the first occurrence of a `needle` in `haystack`.
#[inline]
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    memmem::find(haystack, needle)
}

impl Decoder for JpegCodec {
    type Item = Bytes;
    type Error = io::Error;

    /// Attempt to decode one complete JPEG frame from `src`.
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
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
}

impl Encoder<&[u8]> for JpegCodec {
    type Error = io::Error;

    /// Not used for sending frames in this example, so do nothing.
    fn encode(&mut self, _item: &[u8], _dst: &mut BytesMut) -> Result<(), Self::Error> {
        unimplemented!()
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

        let frame = codec.decode(&mut src).unwrap().unwrap();
        assert_eq!(frame, &b"\xff\xd8\xff\xe0hello world\xff\xd9"[..]);
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
        assert_eq!(frame1, &b"\xff\xd8\xff\xe0frame1\xff\xd9"[..]);

        let frame2 = codec.decode(&mut src).unwrap().unwrap();
        assert_eq!(frame2, &b"\xff\xd8\xff\xe0frame2\xff\xd9"[..]);

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
}
