use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use wificalling_location_gateway::runtime::uds::{FrameError, FramedIo, CONTROL_FRAME_TIMEOUT};
use wificalling_location_gateway::service::api::MAX_CONTROL_FRAME_BYTES;

fn encoded(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

struct PrefixThenPendingReader {
    prefix: Vec<u8>,
    offset: usize,
    polls: Arc<AtomicUsize>,
}

impl AsyncRead for PrefixThenPendingReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        destination: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.offset == self.prefix.len() {
            return Poll::Pending;
        }
        let count = destination.remaining().min(self.prefix.len() - self.offset);
        let end = self.offset + count;
        destination.put_slice(&self.prefix[self.offset..end]);
        self.offset = end;
        Poll::Ready(Ok(()))
    }
}

struct PartialThenPendingWriter {
    remaining: usize,
    written: Arc<Mutex<Vec<u8>>>,
    polls: Arc<AtomicUsize>,
}

impl AsyncWrite for PartialThenPendingWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.remaining == 0 {
            return Poll::Pending;
        }
        let count = self.remaining.min(buffer.len());
        self.written
            .lock()
            .unwrap()
            .extend_from_slice(&buffer[..count]);
        self.remaining -= count;
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

struct FlushErrorWriter {
    written: Arc<Mutex<Vec<u8>>>,
    polls: Arc<AtomicUsize>,
}

impl AsyncWrite for FlushErrorWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        self.written.lock().unwrap().extend_from_slice(buffer);
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "secret flush detail",
        )))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

struct ErrorReader;

impl AsyncRead for ErrorReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _destination: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "secret read detail",
        )))
    }
}

#[tokio::test]
async fn reads_partial_input_and_two_consecutive_normal_frames() {
    let mut bytes = encoded(b"first");
    bytes.extend_from_slice(&encoded(b"second"));
    let mut source = bytes.as_slice();
    let mut framed = FramedIo::new(&mut source);

    assert_eq!(framed.read_frame().await.unwrap(), b"first");
    assert_eq!(framed.read_frame().await.unwrap(), b"second");
}

#[tokio::test]
async fn empty_or_oversized_header_poisons_without_consuming_body() {
    for length in [0_u32, MAX_CONTROL_FRAME_BYTES as u32 + 1] {
        let polls = Arc::new(AtomicUsize::new(0));
        let mut bytes = length.to_be_bytes().to_vec();
        bytes.extend_from_slice(b"must-not-be-interpreted");
        let reader = PrefixThenPendingReader {
            prefix: bytes,
            offset: 0,
            polls: Arc::clone(&polls),
        };
        let mut framed = FramedIo::new(reader);

        let expected = if length == 0 {
            FrameError::Empty
        } else {
            FrameError::TooLarge { declared: length }
        };
        assert_eq!(framed.read_frame().await, Err(expected));
        let polls_after_error = polls.load(Ordering::SeqCst);
        assert_eq!(
            framed.read_frame().await,
            Err(FrameError::ConnectionPoisoned)
        );
        assert_eq!(polls.load(Ordering::SeqCst), polls_after_error);
    }
}

#[tokio::test]
async fn eof_and_truncation_poison_the_connection_without_payload_in_errors() {
    for (bytes, expected) in [
        (Vec::new(), FrameError::Eof),
        (vec![0, 0], FrameError::TruncatedHeader),
        (encoded(b"abc")[..6].to_vec(), FrameError::TruncatedBody),
    ] {
        let mut source = bytes.as_slice();
        let mut framed = FramedIo::new(&mut source);
        let error = framed.read_frame().await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("abc"));
        assert_eq!(
            framed.read_frame().await,
            Err(FrameError::ConnectionPoisoned)
        );
    }
}

#[tokio::test]
async fn writes_big_endian_frames_and_validation_errors_poison() {
    let mut output = Vec::new();
    let mut framed = FramedIo::new(&mut output);
    framed.write_frame(b"status.get").await.unwrap();
    assert_eq!(output, encoded(b"status.get"));

    let mut empty_output = Vec::new();
    let mut empty = FramedIo::new(&mut empty_output);
    assert_eq!(empty.write_frame(b"").await, Err(FrameError::Empty));
    assert_eq!(
        empty.write_frame(b"later").await,
        Err(FrameError::ConnectionPoisoned)
    );

    let mut large_output = Vec::new();
    let mut large = FramedIo::new(&mut large_output);
    assert_eq!(
        large
            .write_frame(&vec![0; MAX_CONTROL_FRAME_BYTES + 1])
            .await,
        Err(FrameError::TooLarge {
            declared: MAX_CONTROL_FRAME_BYTES as u32 + 1,
        })
    );
    assert_eq!(
        large.write_frame(b"later").await,
        Err(FrameError::ConnectionPoisoned)
    );
}

#[tokio::test]
async fn partial_header_and_partial_body_each_use_one_total_deadline_then_poison() {
    assert_eq!(CONTROL_FRAME_TIMEOUT, Duration::from_secs(2));
    for prefix in [vec![0, 0], encoded(b"abc")[..5].to_vec()] {
        let polls = Arc::new(AtomicUsize::new(0));
        let reader = PrefixThenPendingReader {
            prefix,
            offset: 0,
            polls: Arc::clone(&polls),
        };
        let mut framed = FramedIo::new(reader);
        let started = Instant::now();
        assert_eq!(framed.read_frame().await, Err(FrameError::Deadline));
        assert!(started.elapsed() >= CONTROL_FRAME_TIMEOUT);
        let polls_after_timeout = polls.load(Ordering::SeqCst);
        assert_eq!(
            framed.read_frame().await,
            Err(FrameError::ConnectionPoisoned)
        );
        assert_eq!(polls.load(Ordering::SeqCst), polls_after_timeout);
    }
}

#[tokio::test]
async fn partial_write_uses_one_total_deadline_then_poisons() {
    let polls = Arc::new(AtomicUsize::new(0));
    let written = Arc::new(Mutex::new(Vec::new()));
    let writer = PartialThenPendingWriter {
        remaining: 5,
        written: Arc::clone(&written),
        polls: Arc::clone(&polls),
    };
    let mut framed = FramedIo::new(writer);
    assert_eq!(
        framed.write_frame(b"partial").await,
        Err(FrameError::Deadline)
    );
    let polls_after_timeout = polls.load(Ordering::SeqCst);
    assert_eq!(
        framed.write_frame(b"later").await,
        Err(FrameError::ConnectionPoisoned)
    );
    assert_eq!(polls.load(Ordering::SeqCst), polls_after_timeout);
    assert_eq!(written.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn slow_trickle_cannot_reset_the_single_frame_deadline() {
    let (mut sender, receiver) = tokio::io::duplex(8);
    let sender_task = tokio::spawn(async move {
        for byte in encoded(b"x") {
            tokio::time::sleep(Duration::from_millis(450)).await;
            if sender.write_all(&[byte]).await.is_err() {
                return;
            }
        }
    });
    let mut framed = FramedIo::new(receiver);
    let started = Instant::now();
    assert_eq!(framed.read_frame().await, Err(FrameError::Deadline));
    assert!(started.elapsed() < Duration::from_millis(2300));
    sender_task.abort();
    let _ = sender_task.await;
}

#[tokio::test]
async fn flush_error_is_sanitized_and_poisons_without_more_io() {
    let polls = Arc::new(AtomicUsize::new(0));
    let writer = FlushErrorWriter {
        written: Arc::new(Mutex::new(Vec::new())),
        polls: Arc::clone(&polls),
    };
    let mut framed = FramedIo::new(writer);
    let error = framed.write_frame(b"frame").await.unwrap_err();
    assert_eq!(error, FrameError::Io(io::ErrorKind::PermissionDenied));
    assert!(!error.to_string().contains("secret flush detail"));
    let polls_after_error = polls.load(Ordering::SeqCst);
    assert_eq!(
        framed.write_frame(b"later").await,
        Err(FrameError::ConnectionPoisoned)
    );
    assert_eq!(polls.load(Ordering::SeqCst), polls_after_error);
}

#[tokio::test]
async fn read_io_error_exposes_only_error_kind_and_poisons() {
    let mut framed = FramedIo::new(ErrorReader);
    let error = framed.read_frame().await.unwrap_err();
    assert_eq!(error, FrameError::Io(io::ErrorKind::PermissionDenied));
    assert!(!error.to_string().contains("secret read detail"));
    assert_eq!(
        framed.read_frame().await,
        Err(FrameError::ConnectionPoisoned)
    );
}
