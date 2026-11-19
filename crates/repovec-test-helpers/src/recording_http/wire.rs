//! Reading requests and writing responses on a loopback connection.
//!
//! The server speaks the smallest useful slice of HTTP/1.1: a request line, a
//! header block, and a response with a `Content-Length`. Nothing here knows
//! about the script; it only turns bytes into [`RecordedRequest`] values and
//! [`ScriptedResponse`] values into bytes.

use std::{
    io::{self, BufRead, BufReader, Write},
    net::TcpStream,
};

use super::expectation::{RecordedRequest, ScriptedResponse};

/// The placeholder a scripted header uses to name the server's own origin.
///
/// A `next` link must name the origin the server actually listens on, but that
/// origin is only known once the listener is bound. Writing the placeholder
/// keeps the script declarative.
pub const ORIGIN_TOKEN: &str = "{origin}";

/// Reads one request head, returning `None` when the connection closes.
pub(super) fn read_request(reader: &mut BufReader<TcpStream>) -> Option<RecordedRequest> {
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    Some(RecordedRequest {
        method,
        target,
        headers: read_headers(reader)?,
    })
}

/// Reads a header block up to and including its terminating blank line.
fn read_headers(reader: &mut BufReader<TcpStream>) -> Option<Vec<(String, String)>> {
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            return Some(headers);
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
}

/// Writes one scripted response, resolving the origin placeholder.
pub(super) fn write_response(
    stream: &mut TcpStream,
    origin: &str,
    response: &ScriptedResponse,
) -> io::Result<()> {
    if !response.delay.is_zero() {
        std::thread::sleep(response.delay);
    }
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason_phrase(response.status)
    );
    for (name, value) in &response.headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(&value.replace(ORIGIN_TOKEN, origin));
        head.push_str("\r\n");
    }
    head.push_str("Content-Length: ");
    head.push_str(&response.body.len().to_string());
    head.push_str("\r\n\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

/// Returns the reason phrase for a status code the tests use.
const fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}
