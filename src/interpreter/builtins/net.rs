//! HTTP/HTTPS networking builtins — `download.file()` and `url()`.
//!
//! Provides TLS support via `rustls` with system certificate trust
//! (`rustls-native-certs`) and Mozilla root certificates (`webpki-roots`).
//!
//! `download.file(url, destfile)` performs a one-shot HTTP/HTTPS GET request
//! and writes the response body to a local file.
//!
//! `url(description)` creates a URL connection object that can be read
//! with `readLines()`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use rustls::pki_types::ServerName;

use super::CallArgs;
use crate::interpreter::builtins::connections::ConnectionInfo;
use crate::interpreter::value::*;
use crate::interpreter::BuiltinContext;
use minir_macros::interpreter_builtin;

// region: URL parsing

/// Parsed URL components for HTTP/HTTPS.
struct ParsedUrl {
    scheme: Scheme,
    host: String,
    port: u16,
    path: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scheme {
    Http,
    Https,
}

/// Parse a URL into its components. Only supports http:// and https://.
fn parse_url(url: &str) -> Result<ParsedUrl, RError> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (Scheme::Https, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (Scheme::Http, rest)
    } else {
        return Err(RError::new(
            RErrorKind::Argument,
            format!(
                "unsupported URL scheme in '{}' — only http:// and https:// are supported",
                url
            ),
        ));
    };

    // Split host from path
    let (host_port, path) = match rest.find('/') {
        Some(pos) => (&rest[..pos], &rest[pos..]),
        None => (rest, "/"),
    };

    // Split host from port
    let (host, port) = if let Some(colon_pos) = host_port.rfind(':') {
        let port_str = &host_port[colon_pos + 1..];
        let port: u16 = port_str.parse().map_err(|_| {
            RError::new(
                RErrorKind::Argument,
                format!("invalid port number '{}' in URL '{}'", port_str, url),
            )
        })?;
        (host_port[..colon_pos].to_string(), port)
    } else {
        let default_port = match scheme {
            Scheme::Http => 80,
            Scheme::Https => 443,
        };
        (host_port.to_string(), default_port)
    };

    if host.is_empty() {
        return Err(RError::new(
            RErrorKind::Argument,
            format!("empty host in URL '{}'", url),
        ));
    }

    Ok(ParsedUrl {
        scheme,
        host,
        port,
        path: path.to_string(),
    })
}

// endregion

// region: TLS client configuration

/// Build a rustls `ClientConfig` using system certificates (via rustls-native-certs)
/// with Mozilla roots as fallback (via webpki-roots).
fn tls_client_config() -> Result<Arc<rustls::ClientConfig>, RError> {
    let mut root_store = rustls::RootCertStore::empty();

    // Try loading system certificates first.
    let cert_result = rustls_native_certs::load_native_certs();
    for cert in cert_result.certs {
        // Ignore individual cert errors — some system certs may be
        // unparseable but that shouldn't block the whole store.
        let _ = root_store.add(cert);
    }

    // If no system certs loaded, use Mozilla roots as fallback.
    if root_store.is_empty() {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

/// Open a TCP connection and optionally wrap it in TLS.
/// Returns a boxed Read+Write stream.
fn connect_stream(parsed: &ParsedUrl) -> Result<Box<dyn ReadWriteStream>, RError> {
    let tcp = TcpStream::connect((parsed.host.as_str(), parsed.port)).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!(
                "cannot connect to {}:{} — {}. \
                 Check that the host is reachable and the port is open.",
                parsed.host, parsed.port, e
            ),
        )
    })?;

    match parsed.scheme {
        Scheme::Http => Ok(Box::new(tcp)),
        Scheme::Https => {
            let config = tls_client_config()?;
            let server_name = ServerName::try_from(parsed.host.clone()).map_err(|e| {
                RError::new(
                    RErrorKind::Argument,
                    format!("invalid server name '{}': {}", parsed.host, e),
                )
            })?;
            let conn = rustls::ClientConnection::new(config, server_name).map_err(|e| {
                RError::new(
                    RErrorKind::Other,
                    format!("TLS handshake failed for '{}': {}", parsed.host, e),
                )
            })?;
            let tls_stream = rustls::StreamOwned::new(conn, tcp);
            Ok(Box::new(tls_stream))
        }
    }
}

/// Trait alias for streams that are both Read and Write.
trait ReadWriteStream: Read + Write {}
impl<T: Read + Write> ReadWriteStream for T {}

// endregion

// region: HTTP helpers

/// Perform an HTTP GET request on the given stream, returning the response body as bytes.
fn http_get(stream: &mut dyn ReadWriteStream, host: &str, path: &str) -> Result<Vec<u8>, RError> {
    // Send HTTP/1.1 GET request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: miniR/0.1\r\nAccept: */*\r\n\r\n",
        path, host
    );
    stream.write_all(request.as_bytes()).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to send HTTP request: {}", e),
        )
    })?;

    // Read entire response
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("failed to read HTTP response: {}", e),
        )
    })?;

    // Parse response: find end of headers (blank line)
    let header_end = find_header_end(&response).ok_or_else(|| {
        RError::new(
            RErrorKind::Other,
            "malformed HTTP response — could not find end of headers".to_string(),
        )
    })?;

    let header_bytes = &response[..header_end];
    let header_str = String::from_utf8_lossy(header_bytes);

    // Check status line
    let status_line = header_str.lines().next().unwrap_or("");
    let status_code = parse_status_code(status_line);

    // Handle redirects (301, 302, 303, 307, 308)
    if matches!(status_code, Some(301 | 302 | 303 | 307 | 308)) {
        // Find Location header
        let location = header_str.lines().find_map(|line| {
            if line.to_ascii_lowercase().starts_with("location:") {
                Some(line[9..].trim().to_string())
            } else {
                None
            }
        });

        if let Some(redirect_url) = location {
            return Err(RError::new(
                RErrorKind::Other,
                format!(
                    "HTTP redirect to '{}' — redirect following is not yet supported. \
                     Try using the redirected URL directly.",
                    redirect_url
                ),
            ));
        }
    }

    // Check for error status
    if let Some(code) = status_code {
        if code >= 400 {
            return Err(RError::new(
                RErrorKind::Other,
                format!("HTTP request failed with status {} — {}", code, status_line),
            ));
        }
    }

    // Return body (everything after headers + \r\n\r\n)
    let body_start = header_end + 4; // skip \r\n\r\n
    if body_start <= response.len() {
        Ok(response[body_start..].to_vec())
    } else {
        Ok(Vec::new())
    }
}

/// Find the position of \r\n\r\n in bytes (end of HTTP headers).
fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse the HTTP status code from a status line like "HTTP/1.1 200 OK".
fn parse_status_code(status_line: &str) -> Option<u16> {
    let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
    if parts.len() >= 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}

// endregion

// region: download.file builtin

/// Download a file from a URL.
///
/// Performs an HTTP or HTTPS GET request and writes the response body
/// to the specified local file. Returns 0 (success) or non-zero (failure),
/// matching R's `download.file()` return convention.
///
/// @param url character scalar: the URL to download from
/// @param destfile character scalar: the local file path to write to
/// @param method character scalar: download method (ignored, always "internal")
/// @param quiet logical scalar: if TRUE, suppress progress messages (default FALSE)
/// @return integer scalar: 0 for success
#[interpreter_builtin(name = "download.file", min_args = 2, namespace = "net")]
fn interp_download_file(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let url_str = call_args.string("url", 0)?;
    let destfile = call_args.string("destfile", 1)?;
    let quiet = call_args.logical_flag("quiet", 3, false);

    let parsed = parse_url(&url_str)?;

    if !quiet {
        // R prints a message about the download
        context.write_err(&format!("trying URL '{}'\n", url_str));
    }

    let mut stream = connect_stream(&parsed)?;
    let body = http_get(stream.as_mut(), &parsed.host, &parsed.path)?;

    // Resolve destfile relative to interpreter working directory
    let interp = context.interpreter();
    let dest_path = if std::path::Path::new(&destfile).is_absolute() {
        std::path::PathBuf::from(&destfile)
    } else {
        let wd = interp.get_working_dir();
        wd.join(&destfile)
    };

    std::fs::write(&dest_path, &body).map_err(|e| {
        RError::new(
            RErrorKind::Other,
            format!("cannot write to '{}': {}", dest_path.display(), e),
        )
    })?;

    if !quiet {
        context.write_err(&format!("Content length {} bytes\n", body.len()));
        context.write_err(&format!("downloaded {} bytes\n", body.len()));
    }

    // Return 0 for success (R convention)
    Ok(RValue::vec(Vector::Integer(vec![Some(0)].into())))
}

// endregion

// region: url connection builtin

/// Create a URL connection.
///
/// Opens an HTTP or HTTPS connection to the specified URL. The connection
/// can be read with `readLines()`. Only "r" (read) mode is supported.
///
/// When opened, the connection performs an HTTP GET request and buffers
/// the response body for subsequent reads.
///
/// @param description character scalar: the URL to connect to
/// @param open character scalar: open mode ("" or "r")
/// @return integer scalar with class "connection"
#[interpreter_builtin(name = "url", min_args = 1)]
fn interp_url(
    args: &[RValue],
    named: &[(String, RValue)],
    context: &BuiltinContext,
) -> Result<RValue, RError> {
    let call_args = CallArgs::new(args, named);
    let url_str = call_args.string("description", 0)?;
    let open_mode = call_args.optional_string("open", 1).unwrap_or_default();

    // Validate the URL scheme
    let parsed = parse_url(&url_str)?;

    let interp = context.interpreter();

    // Create a URL connection
    let mut info = ConnectionInfo::url_connection(url_str.clone());

    if !open_mode.is_empty() {
        if open_mode != "r" && open_mode != "rt" && open_mode != "rb" {
            return Err(RError::new(
                RErrorKind::Argument,
                format!(
                    "url() connections only support read mode ('r', 'rt', 'rb'), got '{}'",
                    open_mode
                ),
            ));
        }
        info.mode = open_mode;
        info.is_open = true;

        // Eagerly fetch the content when opening
        let mut stream = connect_stream(&parsed)?;
        let body = http_get(stream.as_mut(), &parsed.host, &parsed.path)?;

        let id = interp.add_connection(info);
        interp.store_url_body(id, body);
        Ok(connection_value(id))
    } else {
        // Create connection but don't open it yet
        let id = interp.add_connection(info);
        Ok(connection_value(id))
    }
}

// endregion

// region: Helper — build a connection RValue

/// Build an integer scalar with class `"connection"` representing connection `id`.
fn connection_value(id: usize) -> RValue {
    let mut rv = RVector::from(Vector::Integer(
        vec![Some(i64::try_from(id).unwrap_or(0))].into(),
    ));
    rv.set_attr(
        "class".to_string(),
        RValue::vec(Vector::Character(
            vec![Some("connection".to_string())].into(),
        )),
    );
    RValue::Vector(rv)
}

// endregion
