//! Length-delimited control frames for a future local Unix-domain socket.
//!
//! Every frame operation is bounded by [`MAX_CONTROL_FRAME_BYTES`] and a single
//! total [`CONTROL_FRAME_TIMEOUT`]. A frame operation that fails for any reason
//! poisons the [`FramedIo`] connection: subsequent operations return
//! [`FrameError::ConnectionPoisoned`] without touching the underlying I/O, so a
//! half-consumed or hostile peer can never resume mid-frame. I/O errors surface
//! only their [`io::ErrorKind`]; underlying messages are never forwarded.
//!
//! This module deliberately contains no socket creation or listener policy.

use std::fmt;
use std::io;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

pub const MAX_CONTROL_FRAME_BYTES: usize = 16 * 1024;
pub const CONTROL_FRAME_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    Empty,
    TooLarge { declared: u32 },
    Eof,
    TruncatedHeader,
    TruncatedBody,
    Deadline,
    Io(io::ErrorKind),
    ConnectionPoisoned,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("control frame is empty"),
            Self::TooLarge { declared } => {
                write!(formatter, "control frame length {declared} exceeds limit")
            }
            Self::Eof => formatter.write_str("control stream reached end of file"),
            Self::TruncatedHeader => formatter.write_str("control frame header is truncated"),
            Self::TruncatedBody => formatter.write_str("control frame body is truncated"),
            Self::Deadline => formatter.write_str("control frame I/O deadline exceeded"),
            Self::Io(kind) => write!(formatter, "control frame I/O failed ({kind:?})"),
            Self::ConnectionPoisoned => {
                formatter.write_str("control frame connection is poisoned after a prior error")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Length-delimited frame codec over a bounded async stream.
///
/// Once a frame operation fails the connection is poisoned; no further I/O is
/// attempted and every subsequent call returns [`FrameError::ConnectionPoisoned`].
pub struct FramedIo<T> {
    io: T,
    poisoned: bool,
}

impl<T> FramedIo<T> {
    /// Wrap a readable and/or writable async I/O resource with the frame codec.
    pub const fn new(io: T) -> Self {
        Self {
            io,
            poisoned: false,
        }
    }

    /// Returns `true` once a prior frame operation has poisoned the connection.
    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

impl<T> FramedIo<T>
where
    T: AsyncRead + Unpin,
{
    /// Read one length-prefixed frame within the single total frame deadline.
    ///
    /// The deadline covers the entire header plus body and cannot be reset by a
    /// slow trickle of bytes. Any error poisons the connection.
    pub async fn read_frame(&mut self) -> Result<Vec<u8>, FrameError> {
        if self.poisoned {
            return Err(FrameError::ConnectionPoisoned);
        }
        match timeout(CONTROL_FRAME_TIMEOUT, read_frame_inner(&mut self.io)).await {
            Ok(Ok(frame)) => Ok(frame),
            Ok(Err(error)) => {
                self.poisoned = true;
                Err(error)
            }
            Err(_) => {
                self.poisoned = true;
                Err(FrameError::Deadline)
            }
        }
    }
}

impl<T> FramedIo<T>
where
    T: AsyncWrite + Unpin,
{
    /// Write one length-prefixed frame within the single total frame deadline.
    ///
    /// Empty and oversized payloads are rejected before any I/O and also poison
    /// the connection. Any I/O or deadline error poisons the connection.
    pub async fn write_frame(&mut self, payload: &[u8]) -> Result<(), FrameError> {
        if self.poisoned {
            return Err(FrameError::ConnectionPoisoned);
        }
        if payload.is_empty() {
            self.poisoned = true;
            return Err(FrameError::Empty);
        }
        if payload.len() > MAX_CONTROL_FRAME_BYTES {
            self.poisoned = true;
            return Err(FrameError::TooLarge {
                declared: payload.len().min(u32::MAX as usize) as u32,
            });
        }
        match timeout(
            CONTROL_FRAME_TIMEOUT,
            write_frame_inner(&mut self.io, payload),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.poisoned = true;
                Err(error)
            }
            Err(_) => {
                self.poisoned = true;
                Err(FrameError::Deadline)
            }
        }
    }
}

async fn read_frame_inner<R>(reader: &mut R) -> Result<Vec<u8>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0_u8; 4];
    let header_bytes = read_exact_counted(reader, &mut header).await?;
    match header_bytes {
        0 => return Err(FrameError::Eof),
        4 => {}
        _ => return Err(FrameError::TruncatedHeader),
    }

    let declared = u32::from_be_bytes(header);
    if declared == 0 {
        return Err(FrameError::Empty);
    }
    if declared as usize > MAX_CONTROL_FRAME_BYTES {
        return Err(FrameError::TooLarge { declared });
    }

    let mut payload = vec![0_u8; declared as usize];
    if read_exact_counted(reader, &mut payload).await? != payload.len() {
        return Err(FrameError::TruncatedBody);
    }
    Ok(payload)
}

async fn write_frame_inner<W>(writer: &mut W, payload: &[u8]) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    let header = (payload.len() as u32).to_be_bytes();
    writer
        .write_all(&header)
        .await
        .map_err(|error| FrameError::Io(error.kind()))?;
    writer
        .write_all(payload)
        .await
        .map_err(|error| FrameError::Io(error.kind()))?;
    writer
        .flush()
        .await
        .map_err(|error| FrameError::Io(error.kind()))
}

async fn read_exact_counted<R>(reader: &mut R, destination: &mut [u8]) -> Result<usize, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut read = 0;
    while read < destination.len() {
        let count = reader
            .read(&mut destination[read..])
            .await
            .map_err(|error| FrameError::Io(error.kind()))?;
        if count == 0 {
            break;
        }
        read += count;
    }
    Ok(read)
}
