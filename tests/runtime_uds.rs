use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use wificalling_location_gateway::runtime::uds::{
    read_frame, write_frame, FrameError, CONTROL_FRAME_TIMEOUT, MAX_CONTROL_FRAME_BYTES,
};

struct ChunkedReader {
    bytes: Vec<u8>,
    offset: usize,
    chunk_size: usize,
}

impl AsyncRead for ChunkedReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        destination: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.offset == self.bytes.len() {
            return Poll::Ready(Ok(()));
        }

        let end = (self.offset + self.chunk_size).min(self.bytes.len());
        destination.put_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Poll::Ready(Ok(()))
    }
}

struct PendingIo;

impl AsyncRead for PendingIo {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _destination: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}

impl AsyncWrite for PendingIo {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn encoded(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

#[tokio::test]
async fn reads_big_endian_frame_across_partial_reads() {
    let payload = b"{\"version\":\"wloc.service/v1\"}";
    let mut reader = ChunkedReader {
        bytes: encoded(payload),
        offset: 0,
        chunk_size: 1,
    };

    assert_eq!(read_frame(&mut reader).await.unwrap(), payload);
}

#[tokio::test]
async fn rejects_empty_and_oversized_headers_without_reading_a_body() {
    for (length, expected) in [
        (0_u32, FrameError::Empty),
        (
            MAX_CONTROL_FRAME_BYTES as u32 + 1,
            FrameError::TooLarge {
                declared: MAX_CONTROL_FRAME_BYTES as u32 + 1,
            },
        ),
    ] {
        let mut reader = ChunkedReader {
            bytes: length.to_be_bytes().to_vec(),
            offset: 0,
            chunk_size: 1,
        };
        assert_eq!(read_frame(&mut reader).await, Err(expected));
        assert_eq!(reader.offset, 4, "body must not be read after invalid header");
    }
}

#[tokio::test]
async fn reports_clean_eof_and_truncated_header_or_body_without_payload_data() {
    for (bytes, expected) in [
        (Vec::new(), FrameError::Eof),
        (vec![0, 0], FrameError::TruncatedHeader),
        (encoded(b"abc")[..6].to_vec(), FrameError::TruncatedBody),
    ] {
        let mut reader = bytes.as_slice();
        let error = read_frame(&mut reader).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("abc"));
    }
}

#[tokio::test]
async fn writes_big_endian_frame_and_rejects_empty_or_oversized_payloads() {
    let (mut writer, mut reader) = tokio::io::duplex(MAX_CONTROL_FRAME_BYTES + 4);
    let payload = b"status.get";
    write_frame(&mut writer, payload).await.unwrap();
    let mut observed = vec![0; payload.len() + 4];
    reader.read_exact(&mut observed).await.unwrap();
    assert_eq!(observed, encoded(payload));

    assert_eq!(write_frame(&mut writer, b"").await, Err(FrameError::Empty));
    assert_eq!(
        write_frame(&mut writer, &vec![0; MAX_CONTROL_FRAME_BYTES + 1]).await,
        Err(FrameError::TooLarge {
            declared: MAX_CONTROL_FRAME_BYTES as u32 + 1,
        })
    );
}

#[tokio::test]
async fn read_and_write_each_enforce_one_total_two_second_deadline() {
    assert_eq!(CONTROL_FRAME_TIMEOUT, Duration::from_secs(2));

    let started = Instant::now();
    assert_eq!(read_frame(&mut PendingIo).await, Err(FrameError::Deadline));
    assert!(started.elapsed() >= CONTROL_FRAME_TIMEOUT);

    let started = Instant::now();
    assert_eq!(
        write_frame(&mut PendingIo, b"bounded").await,
        Err(FrameError::Deadline)
    );
    assert!(started.elapsed() >= CONTROL_FRAME_TIMEOUT);
}

#[tokio::test]
async fn supports_generic_async_read_and_write_implementations() {
    async fn generic_roundtrip<R, W>(reader: &mut R, writer: &mut W)
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let frame = read_frame(reader).await.unwrap();
        write_frame(writer, &frame).await.unwrap();
    }

    let payload = b"generic";
    let mut source = encoded(payload).as_slice();
    let mut destination = Vec::new();
    generic_roundtrip(&mut source, &mut destination).await;
    assert_eq!(destination, encoded(payload));
}
