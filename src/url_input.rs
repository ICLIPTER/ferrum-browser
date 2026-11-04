//! Address-bar parsing and validation.
//!
//! A browser address bar accepts more than strict URLs. Users type
//! `example.com`, paste `https://docs.rs/eframe`, or occasionally something
//! that is not an address at all. This module turns that raw text into a
//! valid [`Url`], or explains why it cannot, so the UI layer stays simple.

use std::fmt;

use url::Url;

/// Schemes that are recognisably not `http`/`https` and should be reported as
/// unsupported instead of being mistaken for a host name.
const NON_WEB_SCHEMES: [&str; 6] = ["mailto:", "tel:", "javascript:", "data:", "about:", "file:"];

/// Everything that can go wrong while interpreting address-bar text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressError {
    /// The user submitted an empty (or whitespace-only) string.
    Empty,
    /// URLs cannot contain whitespace.
    Spaces,
    /// Looks more like a search term than a host name.
    NotAnAddress,
    /// The URL parsed, but has no host to connect to.
    MissingHost,
    /// A scheme Ferrum Browser does not handle yet.
    UnsupportedScheme(String),
    /// The `url` crate rejected the input.
    Invalid(String),
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Type a web address to get started."),
            Self::Spaces => write!(
                f,
                "Web addresses can't contain spaces. Try something like example.com."
            ),
            Self::NotAnAddress => write!(
                f,
                "That doesn't look like a web address. Try example.com or https://example.com."
            ),
            Self::MissingHost => write!(f, "That address is missing a host name."),
            Self::UnsupportedScheme(scheme) => write!(
                f,
                "Ferrum can't open \"{scheme}\" links yet — only http:// and https:// are supported."
            ),
            Self::Invalid(reason) => write!(f, "Invalid address: {reason}."),
        }
    }
}

impl std::error::Error for AddressError {}

/// Parse raw address-bar text into a URL.
///
/// Rules, in order:
/// 1. Trim surrounding whitespace; reject empty input.
/// 2. Reject internal whitespace outright.
/// 3. If the text has no explicit scheme, assume `https://`.
/// 4. Bare text with no dot, colon, or `localhost` is probably a search term,
///    so reject it with a hint instead of silently building `https://hello`.
/// 5. Only `http` and `https` are accepted (Phase 1 has no other handlers).
pub fn parse_address(input: &str) -> Result<Url, AddressError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(AddressError::Empty);
    }

    if trimmed.chars().any(char::is_whitespace) {
        return Err(AddressError::Spaces);
    }

    let explicit_scheme = trimmed.contains("://")
        || NON_WEB_SCHEMES
            .iter()
            .any(|prefix| trimmed.to_ascii_lowercase().starts_with(prefix));

    if !explicit_scheme && !looks_like_host(trimmed) {
        return Err(AddressError::NotAnAddress);
    }

    let candidate = if explicit_scheme {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };

    let url = Url::parse(&candidate).map_err(|err| AddressError::Invalid(err.to_string()))?;

    match url.scheme() {
        "http" | "https" => {}
        other => return Err(AddressError::UnsupportedScheme(other.to_owned())),
    }

    if url.host_str().is_none_or(str::is_empty) {
        return Err(AddressError::MissingHost);
    }

    Ok(url)
}

/// Heuristic for "this was meant as an address, not a search query".
///
/// Dots (domain names and IPv4), colons (ports and IPv6) and `localhost` all
/// count. Anything else — say, `rust tutorial` or `hello` — gets the hint.
fn looks_like_host(input: &str) -> bool {
    let without_user_info = input.rsplit('@').next().unwrap_or(input);
    let host_and_port = without_user_info
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_user_info);

    host_and_port.eq_ignore_ascii_case("localhost")
        || host_and_port.contains('.')
        || host_and_port.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse_address("   "), Err(AddressError::Empty));
    }

    #[test]
    fn rejects_spaces() {
        assert_eq!(parse_address("rust tutorial"), Err(AddressError::Spaces));
    }

    #[test]
    fn rejects_search_terms() {
        assert_eq!(parse_address("hello"), Err(AddressError::NotAnAddress));
    }

    #[test]
    fn adds_https_to_bare_domains() {
        let url = parse_address("example.com").expect("valid address");
        assert_eq!(url.as_str(), "https://example.com/");
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let url = parse_address("  example.com  ").expect("valid address");
        assert_eq!(url.as_str(), "https://example.com/");
    }

    #[test]
    fn keeps_explicit_http_scheme() {
        let url = parse_address("http://example.com/start").expect("valid address");
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.path(), "/start");
    }

    #[test]
    fn accepts_ports_and_paths() {
        let url = parse_address("localhost:3000/app?tab=1").expect("valid address");
        assert_eq!(url.host_str(), Some("localhost"));
        assert_eq!(url.port(), Some(3000));
        assert_eq!(url.path(), "/app");
        assert_eq!(url.query(), Some("tab=1"));
    }

    #[test]
    fn rejects_unsupported_schemes() {
        assert_eq!(
            parse_address("ftp://example.com"),
            Err(AddressError::UnsupportedScheme("ftp".to_owned()))
        );
    }

    #[test]
    fn rejects_markup_pseudo_urls() {
        let err = parse_address("javascript:alert(1)").expect_err("must be rejected");
        assert!(matches!(err, AddressError::UnsupportedScheme(_)));
    }

    #[test]
    fn rejects_unparseable_input() {
        let err = parse_address("https://exa mple.com").expect_err("must be rejected");
        assert_eq!(err, AddressError::Spaces);
    }
}
