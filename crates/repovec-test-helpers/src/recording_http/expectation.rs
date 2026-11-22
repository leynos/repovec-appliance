//! Scripted responses and the requests a test observes.
//!
//! A scripted server answers one expectation per request, so the expectation
//! and the request that consumed it are the two halves of the same record.
//! They are small value types with no socket in sight, which keeps them
//! testable on their own and out of the server's accept loop.

use std::time::Duration;

/// One request the server received.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRequest {
    /// HTTP method, for example `GET`.
    pub method: String,
    /// Request target, including the query string when present.
    pub target: String,
    /// Header name/value pairs in received order, with names lowercased.
    pub headers: Vec<(String, String)>,
}

impl RecordedRequest {
    /// Returns the first value of the named header, ignoring case.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::recording_http::RecordedRequest;
    ///
    /// let request = RecordedRequest {
    ///     method: "GET".to_owned(),
    ///     target: "/user/repos".to_owned(),
    ///     headers: vec![("accept".to_owned(), "application/json".to_owned())],
    /// };
    ///
    /// assert_eq!(request.header("Accept"), Some("application/json"));
    /// assert_eq!(request.header("authorization"), None);
    /// ```
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Returns the request target's path, without its query string.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::recording_http::RecordedRequest;
    ///
    /// let request = RecordedRequest {
    ///     method: "GET".to_owned(),
    ///     target: "/user/repos?page=2&per_page=100".to_owned(),
    ///     headers: Vec::new(),
    /// };
    ///
    /// assert_eq!(request.path(), "/user/repos");
    /// ```
    #[must_use]
    pub fn path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _)| path)
    }
}

/// One scripted response the server sends.
#[derive(Clone, Debug)]
pub struct ScriptedResponse {
    /// Status code sent in the response line.
    pub(crate) status: u16,
    /// Extra response headers, in the order supplied.
    pub(crate) headers: Vec<(String, String)>,
    /// Response body bytes.
    pub(crate) body: Vec<u8>,
    /// Delay applied before the response is written.
    pub(crate) delay: Duration,
}

impl ScriptedResponse {
    /// Creates a response with the given status, no body, and no delay.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::recording_http::ScriptedResponse;
    ///
    /// let response = ScriptedResponse::new(404);
    ///
    /// assert_eq!(response.status_code(), 404);
    /// ```
    #[must_use]
    pub const fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            delay: Duration::ZERO,
        }
    }

    /// Returns these responses with the supplied body.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::recording_http::ScriptedResponse;
    ///
    /// let response = ScriptedResponse::new(200).with_body("[]");
    ///
    /// assert_eq!(response.body_text(), "[]");
    /// ```
    #[must_use]
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into().into_bytes();
        self
    }

    /// Returns this response with an additional header.
    ///
    /// Repeated calls add repeated headers rather than replacing earlier ones,
    /// so several `Link` headers can be scripted.
    ///
    /// # Examples
    ///
    /// ```
    /// use repovec_test_helpers::recording_http::ScriptedResponse;
    ///
    /// let response = ScriptedResponse::new(200).with_header("Link", "</x>; rel=\"next\"");
    ///
    /// assert_eq!(response.header_count("link"), 1);
    /// ```
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Returns this response with a delay before it is written.
    ///
    /// A delay longer than the client's request timeout exercises the timeout
    /// path without waiting for a production deadline.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use repovec_test_helpers::recording_http::ScriptedResponse;
    ///
    /// let response = ScriptedResponse::new(200).with_delay(Duration::from_millis(50));
    ///
    /// assert_eq!(response.delay(), Duration::from_millis(50));
    /// ```
    #[must_use]
    pub const fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Returns the scripted status code.
    #[must_use]
    pub const fn status_code(&self) -> u16 { self.status }

    /// Returns the scripted delay.
    #[must_use]
    pub const fn delay(&self) -> Duration { self.delay }

    /// Returns the number of scripted headers with the given name.
    #[must_use]
    pub fn header_count(&self, name: &str) -> usize {
        self.headers
            .iter()
            .filter(|(header, _)| header.eq_ignore_ascii_case(name))
            .count()
    }

    /// Returns the body interpreted as UTF-8, or an empty string otherwise.
    #[must_use]
    pub fn body_text(&self) -> &str { std::str::from_utf8(&self.body).unwrap_or_default() }
}
