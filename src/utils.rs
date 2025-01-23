use bytes::Bytes;
use tokio::task;
use turbojpeg::DecompressHeader;

pub(crate) async fn read_jpeg_header(
    jpeg_frame_bytes: Bytes,
) -> turbojpeg::Result<DecompressHeader> {
    task::spawn_blocking(move || turbojpeg::read_header(&jpeg_frame_bytes))
        .await
        .unwrap()
}
