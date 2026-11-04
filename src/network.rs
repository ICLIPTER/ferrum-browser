//! HTTP networking for Ferrum Browser.
//!
//! Phase 2 is deliberately narrow: perform one `GET` request and return one
//! typed result. This module knows nothing about egui or the address bar, so it
//! can be tested on its own. The UI layer will call [`fetch`] from a worker
//! thread in the next step.
//!
//! [`fetch`] is *blocking*: while it waits for the network, the calling thread
//! does nothing else. That is why it must never run on the UI thread.

use std::fmt;
use std::io::Read;
use std::time::{Duration, Instant};

use url::Url;

/// Hard cap on how much of a response body we keep.
///
/// A server can stream for a very long time. Reading at most `2 MiB` keeps one
/// page from exhausting memory; anything beyond that is cut off and reported
/// through [`DownloadedPage::truncated`].
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Give up on a request after this many seconds.
///
/// Without a timeout a broken server could keep the worker thread (and the
/// page load) waiting forever.
pub const TIMEOUT_SECS: u64 = 15;

/// Follow at most this many redirects before reporting an error.
///
/// Servers can redirect in a loop; a limit turns that into an error instead of
/// an endless request chain.
pub const MAX_REDIRECTS: usize = 5;

/// Identifies Ferrum honestly in the `User-Agent` request header.
///
/// `concat!` builds the string at compile time, so this is a `&'static str`
/// with no runtime allocation. The version comes from `Cargo.toml`.
pub const USER_AGENT: &str = concat!("FerrumBrowser/", env!("CARGO_PKG_VERSION"));

/// A successfully downloaded web page, still as raw bytes turned into text.
///
/// This is *not* a parsed document. `body` contains the exact characters the
/// server sent, tags and all.
#[derive(Debug)]
pub struct DownloadedPage {
    /// The URL the caller asked for.
    pub requested_url: Url,
    /// The URL that actually answered, after redirects.
    pub final_url: String,
    /// HTTP status code, for example `200`.
    pub status: u16,
    /// Status text, for example `OK`.
    pub reason: String,
    /// Raw `Content-Type` header, for example `text/html; charset=utf-8`.
    pub content_type: Option<String>,
    /// The response body decoded as UTF-8 text (invalid bytes are replaced).
    pub body: String,
    /// `true` if the body was longer than [`MAX_BODY_BYTES`].
    pub truncated: bool,
    /// How long the request took, including reading the body.
    pub elapsed: Duration,
}

/// Everything that can go wrong while fetching a page.
///
/// Each variant carries enough context to show a useful message in the UI:
/// which URL failed and what the server or network said.
#[derive(Debug)]
pub enum FetchError {
    /// DNS, TCP, TLS, or timeout failure: we never received a response.
    Network {
        /// The URL that could not be reached.
        url: String,
        /// The underlying error, rendered as text.
        reason: String,
    },
    /// We received a response, but its status code was not `2xx`.
    Http {
        /// The URL that returned the error status.
        url: String,
        /// The HTTP status code, for example `404`.
        status: u16,
        /// The server's status text, for example `Not Found`.
        reason: String,
    },
    /// The status line was fine, but reading the body failed.
    Body {
        /// The URL whose body failed to download.
        url: String,
        /// The underlying error, rendered as text.
        reason: String,
    },
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { url, reason } => {
                write!(f, "Could not connect to {url}: {reason}")
            }
            Self::Http {
                url,
                status,
                reason,
            } => {
                write!(f, "{url} returned HTTP {status} {reason}")
            }
            Self::Body { url, reason } => {
                write!(f, "Could not read the response body from {url}: {reason}")
            }
        }
    }
}

impl std::error::Error for FetchError {}

/// Download `url` with a single HTTP `GET` request.
///
/// The request carries a `User-Agent`, an `Accept` header, a
/// [`TIMEOUT_SECS`] timeout, and a redirect limit. Redirects are followed by
/// `minreq`, so `DownloadedPage::final_url` can differ from `url`.
///
/// Returns [`FetchError`] for every failure mode instead of panicking, so the
/// caller decides how to present the problem.
pub fn fetch(url: &Url) -> Result<DownloadedPage, FetchError> {
    let requested_url = url.clone();
    let requested = url.as_str().to_owned();
    let started = Instant::now();

    // `send_lazy` returns as soon as the status line and headers have arrived.
    // The body is then read through the `Read` trait, which lets us stop early.
    let response = minreq::get(url.as_str())
        .with_header("User-Agent", USER_AGENT)
        .with_header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .with_timeout(TIMEOUT_SECS)
        .with_max_redirects(MAX_REDIRECTS)
        .send_lazy()
        .map_err(|error| FetchError::Network {
            url: requested.clone(),
            reason: error.to_string(),
        })?;

    let status = response.status_code;
    let reason = response.reason_phrase.clone();
    let final_url = response.url.clone();
    // `ResponseLazy` has no `header` helper, so search the list directly.
    // Header names are case-insensitive, exactly as HTTP defines them.
    let content_type = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Content-Type"))
        .map(|(_, value)| value.clone());

    // 1xx, 3xx (unresolved), 4xx, and 5xx are all failures for a page load.
    // A 404 response is a valid HTTP exchange but not a usable page.
    if !(200..=299).contains(&status) {
        return Err(FetchError::Http {
            url: final_url,
            status,
            reason,
        });
    }

    // `Read::take` reads at most `MAX_BODY_BYTES + 1` bytes. The extra byte is
    // how we detect that the body was longer, and it also lets us refuse to
    // allocate more than the cap.
    let mut limited = response.take(MAX_BODY_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    limited
        .read_to_end(&mut bytes)
        .map_err(|error| FetchError::Body {
            url: final_url.clone(),
            reason: error.to_string(),
        })?;

    let truncated = bytes.len() > MAX_BODY_BYTES;
    bytes.truncate(MAX_BODY_BYTES);

    // `from_utf8_lossy` never fails: invalid bytes become U+FFFD. A multi-byte
    // character cut off by the size limit turns into one replacement character.
    let body = String::from_utf8_lossy(&bytes).into_owned();

    Ok(DownloadedPage {
        requested_url,
        final_url,
        status,
        reason,
        content_type,
        body,
        truncated,
        elapsed: started.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;
    use std::thread;

    /// Start a local server that answers exactly one request with `raw`.
    ///
    /// Tests use a real socket instead of mocking so the HTTP parsing and the
    /// status handling are exercised for real. Being local, it is fast and does
    /// not need internet access.
    fn serve_once(raw: &'static str) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback port");
        let address = listener.local_addr().expect("read local address");

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Read (and ignore) the request so the client sees us as polite.
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(raw.as_bytes());
                let _ = stream.flush();
            }
        });

        Url::parse(&format!("http://{address}/")).expect("parse test URL")
    }

    /// Start a local server that sends `body_len` bytes of `a`.
    fn serve_large_body(body_len: usize) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback port");
        let address = listener.local_addr().expect("read local address");

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request);

                let headers = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: text/plain\r\n\
                     Content-Length: {body_len}\r\n\
                     Connection: close\r\n\r\n"
                );
                let _ = stream.write_all(headers.as_bytes());

                // The client stops reading at the cap and closes the socket;
                // write errors after that are expected and ignored.
                let chunk = [b'a'; 8192];
                let mut written = 0;
                while written < body_len {
                    let size = (body_len - written).min(chunk.len());
                    if stream.write_all(&chunk[..size]).is_err() {
                        break;
                    }
                    written += size;
                }
            }
        });

        Url::parse(&format!("http://{address}/")).expect("parse test URL")
    }

    const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\n\
                               Content-Type: text/html; charset=utf-8\r\n\
                               Content-Length: 13\r\n\
                               Connection: close\r\n\
                               \r\n\
                               hello, world!";

    const NOT_FOUND_RESPONSE: &str = "HTTP/1.1 404 Not Found\r\n\
                                      Content-Length: 9\r\n\
                                      Connection: close\r\n\
                                      \r\n\
                                      not found";

    #[test]
    fn fetch_reads_status_headers_and_body() {
        let url = serve_once(OK_RESPONSE);
        let page = fetch(&url).expect("fetch should succeed");

        assert_eq!(page.status, 200);
        assert_eq!(page.reason, "OK");
        assert_eq!(page.body, "hello, world!");
        assert_eq!(
            page.content_type.as_deref(),
            Some("text/html; charset=utf-8")
        );
        assert!(!page.truncated);
        assert_eq!(page.requested_url, url);
        assert!(page.final_url.starts_with("http://127.0.0.1:"));
        assert!(page.elapsed < Duration::from_secs(5));
    }

    #[test]
    fn fetch_rejects_http_error_status() {
        let url = serve_once(NOT_FOUND_RESPONSE);
        let error = fetch(&url).expect_err("a 404 must be an error");

        match error {
            FetchError::Http {
                status,
                reason,
                url: error_url,
            } => {
                assert_eq!(status, 404);
                assert_eq!(reason, "Not Found");
                assert!(error_url.starts_with("http://127.0.0.1:"));
            }
            other => panic!("expected FetchError::Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_truncates_oversized_bodies() {
        let url = serve_large_body(MAX_BODY_BYTES + 1024);
        let page = fetch(&url).expect("fetch should succeed");

        assert!(page.truncated);
        assert_eq!(page.body.len(), MAX_BODY_BYTES);
        assert!(page.body.bytes().all(|byte| byte == b'a'));
    }

    #[test]
    fn error_messages_explain_the_problem() {
        let url = "https://example.com/".to_owned();

        let network = FetchError::Network {
            url: url.clone(),
            reason: "connection refused".to_owned(),
        };
        assert_eq!(
            network.to_string(),
            "Could not connect to https://example.com/: connection refused"
        );

        let http = FetchError::Http {
            url: url.clone(),
            status: 500,
            reason: "Internal Server Error".to_owned(),
        };
        assert_eq!(
            http.to_string(),
            "https://example.com/ returned HTTP 500 Internal Server Error"
        );

        let body = FetchError::Body {
            url,
            reason: "connection reset".to_owned(),
        };
        assert_eq!(
            body.to_string(),
            "Could not read the response body from https://example.com/: connection reset"
        );
    }

    /// Needs real internet access, so it is opt-in:
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore = "requires internet access"]
    fn fetch_example_com_over_https() {
        let url = Url::parse("https://example.com/").expect("parse live URL");
        let page = fetch(&url).expect("live fetch should succeed");

        assert_eq!(page.status, 200);
        assert_eq!(page.reason, "OK");
        assert!(page.body.to_ascii_lowercase().contains("<html"));
        assert!(!page.truncated);
    }
}
