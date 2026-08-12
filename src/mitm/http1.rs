//! Minimal HTTP/1.1 upstream client for the MITM proxy.
//!
//! The real Apple `/clls/wloc` endpoint serves HTTP/1.1 (an h2 upstream fails
//! with `frame with invalid size`), so the proxy forwards client requests over
//! HTTP/1.1 and reads a bounded response, decoding `Content-Length` and
//! `Transfer-Encoding: chunked` bodies.

use std::io;

use http::{HeaderName, Request, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::client::TlsStream;

use crate::mitm::proxy::MitmProxyError;

/// Hop-by-hop headers stripped from the forwarded request.
const HOP_BY_HOP: [&str; 7] = [
    "connection",
    "proxy-connection",
    "keep-alive",
    "transfer-encoding",
    "upgrade",
    "te",
    "host",
];

/// Upper bound for the whole HTTP/1.1 response (headers + body).
pub const MAX_HTTP1_RESPONSE_BYTES: usize = 512 * 1024;

/// Forward an HTTP/2 client request to the upstream over HTTP/1.1 and return
/// the parsed response body.
pub async fn forward_http1(
    mut stream: TlsStream<tokio::net::TcpStream>,
    request: &Request<()>,
    request_body: &[u8],
) -> Result<(StatusCode, http::HeaderMap, Vec<u8>), MitmProxyError> {
    let path = request
        .uri()
        .path_and_query()
        .map(|part| part.as_str())
        .unwrap_or("/");

    let mut wire = Vec::new();
    wire.extend_from_slice(format!("{} {path} HTTP/1.1\r\n", request.method()).as_bytes());
    for (name, value) in request.headers() {
        let lower = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&lower.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            wire.extend_from_slice(format!("{lower}: {value}\r\n").as_bytes());
        }
    }
    // Host from the request authority (approved hostname), not hop headers.
    if let Some(authority) = request.uri().authority() {
        wire.extend_from_slice(format!("Host: {authority}\r\n").as_bytes());
    }
    if !request_body.is_empty() {
        wire.extend_from_slice(format!("Content-Length: {}\r\n", request_body.len()).as_bytes());
    }
    wire.extend_from_slice(b"Connection: close\r\n\r\n");

    stream
        .write_all(&wire)
        .await
        .map_err(|error| MitmProxyError::Upstream(error.to_string()))?;
    if !request_body.is_empty() {
        stream
            .write_all(request_body)
            .await
            .map_err(|error| MitmProxyError::Upstream(error.to_string()))?;
    }

    let mut raw = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => break,
            Ok(count) => {
                raw.extend_from_slice(&buffer[..count]);
                if raw.len() > MAX_HTTP1_RESPONSE_BYTES {
                    return Err(MitmProxyError::Upstream(
                        "upstream response exceeds bound".into(),
                    ));
                }
            }
            Err(error) => {
                return Err(MitmProxyError::Upstream(error.to_string()));
            }
        }
    }

    parse_http1_response(&raw)
}

/// Parse an HTTP/1.1 response into status, headers, and a decoded body.
fn parse_http1_response(
    raw: &[u8],
) -> Result<(StatusCode, http::HeaderMap, Vec<u8>), MitmProxyError> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| MitmProxyError::Upstream("malformed HTTP/1.1 response".into()))?;
    let header = &raw[..header_end];
    let body = &raw[header_end + 4..];

    let mut lines = header.split(|byte| *byte == b'\n');
    let status_line = lines
        .next()
        .ok_or_else(|| MitmProxyError::Upstream("empty HTTP/1.1 status".into()))?;
    let status_code: u16 = status_line
        .split(|byte| *byte == b' ')
        .nth(1)
        .and_then(|part| std::str::from_utf8(part).ok())
        .and_then(|part| part.trim().parse().ok())
        .ok_or_else(|| MitmProxyError::Upstream("invalid HTTP/1.1 status".into()))?;

    let mut headers = http::HeaderMap::new();
    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let line = String::from_utf8_lossy(line);
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name == "content-length" {
                content_length = value.parse::<usize>().ok();
            } else if name == "transfer-encoding" && value.to_ascii_lowercase().contains("chunked")
            {
                chunked = true;
            }
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                value.parse::<http::HeaderValue>(),
            ) {
                headers.append(name, value);
            }
        }
    }

    let decoded = if chunked {
        decode_chunked(body)?
    } else if let Some(length) = content_length {
        if body.len() < length {
            return Err(MitmProxyError::Upstream("truncated upstream body".into()));
        }
        body[..length].to_vec()
    } else {
        body.to_vec()
    };

    Ok((
        StatusCode::from_u16(status_code).unwrap_or(StatusCode::BAD_GATEWAY),
        headers,
        decoded,
    ))
}

/// Decode an HTTP/1.1 chunked body.
fn decode_chunked(body: &[u8]) -> Result<Vec<u8>, MitmProxyError> {
    let mut output = Vec::new();
    let mut offset = 0;
    loop {
        // Chunk size line (hex) ends with \r\n.
        let line_end = body[offset..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| MitmProxyError::Upstream("malformed chunk size".into()))?
            + offset;
        let size_text = std::str::from_utf8(&body[offset..line_end])
            .map_err(|_| MitmProxyError::Upstream("malformed chunk size".into()))?;
        let size_text = size_text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| MitmProxyError::Upstream("invalid chunk size".into()))?;
        offset = line_end + 2;
        if size == 0 {
            return Ok(output);
        }
        let end = offset + size;
        if end + 2 > body.len() || &body[end..end + 2] != b"\r\n" {
            return Err(MitmProxyError::Upstream("truncated chunk".into()));
        }
        output.extend_from_slice(&body[offset..end]);
        offset = end + 2;
        if output.len() > MAX_HTTP1_RESPONSE_BYTES {
            return Err(MitmProxyError::Upstream(
                "chunked body exceeds bound".into(),
            ));
        }
    }
}

/// Keep `io` referenced for potential io-level helpers.
#[allow(dead_code)]
fn _io_hint() -> io::Result<()> {
    Ok(())
}
