//! Length-delimited control frames for a future local Unix-domain socket.
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
        }
    }
}

impl std::error::Error for FrameError {}

pub async fn read_frame<R>(reader: &mut R) -> Result<Vec<u8>, FrameError>
where
    R: AsyncRead + Unpin,
{
    timeout(CONTROL_FRAME_TIMEOUT, read_frame_inner(reader))
        .await
        .map_err(|_| FrameError::Deadline)?
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

pub async fn write_frame<W>(writer: &mut W, payload: &[u8]) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    if payload.is_empty() {
        return Err(FrameError::Empty);
    }
    if payload.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            declared: payload.len().min(u32::MAX as usize) as u32,
        });
    }

    timeout(CONTROL_FRAME_TIMEOUT, write_frame_inner(writer, payload))
        .await
        .map_err(|_| FrameError::Deadline)?
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
