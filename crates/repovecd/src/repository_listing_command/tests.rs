//! Unit tests for credential reading and observation writing.
//!
//! These cover the two boundaries the composition owns itself: what standard
//! input may contain, and what happens when an output stream fails part way
//! through a listing. The traversal and the diagnostics are exercised end to
//! end against a loopback server.

use std::io::{self, Write};

use repovec_core::repository_listing::RepositorySummary;
use repovec_test_helpers::repository_listing::RepositoryFixture;

use super::{
    credential::{MAX_INSTALLATION_TOKEN_BYTES, read_installation_token},
    output::write_observations,
};

/// A token value that is header-safe and unmistakably fake.
const TOKEN: &str = "ghs_fake_installation_token";

/// A writer that refuses every byte past a fixed budget.
struct ByteBudgetWriter {
    /// Bytes accepted so far.
    written: Vec<u8>,
    /// Total bytes the writer will accept.
    budget: usize,
}

impl ByteBudgetWriter {
    /// Creates a writer that accepts exactly `budget` bytes.
    const fn new(budget: usize) -> Self {
        Self {
            written: Vec::new(),
            budget,
        }
    }
}

impl Write for ByteBudgetWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.written.len().saturating_add(buffer.len()) > self.budget {
            return Err(io::Error::other("the stream is closed"));
        }
        self.written.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn installation_token_accepts_one_trailing_line_ending() {
    for input in [
        format!("{TOKEN}\n"),
        format!("{TOKEN}\r\n"),
        TOKEN.to_owned(),
    ] {
        let mut stdin = input.as_bytes();
        let token =
            read_installation_token(&mut stdin).expect("one trailing line ending must be accepted");
        assert_eq!(
            token.secret(),
            TOKEN,
            "the line ending must not become part of the token"
        );
    }
}

#[test]
fn installation_token_rejects_a_second_line_ending() {
    let input = format!("{TOKEN}\n\n");
    let mut stdin = input.as_bytes();
    let failure = read_installation_token(&mut stdin)
        .expect_err("only one trailing line ending may be removed");

    assert_eq!(
        failure,
        super::CredentialError::StandardInput {
            reason: "must not contain control characters",
        },
        "a second line ending is a control character inside the token"
    );
}

#[test]
fn installation_token_rejects_empty_input() {
    let mut stdin: &[u8] = &[];
    let failure = read_installation_token(&mut stdin)
        .expect_err("an empty standard input cannot supply a credential");

    assert_eq!(
        failure,
        super::CredentialError::StandardInput {
            reason: "must not be empty",
        },
        "the rejection must name emptiness rather than a later failure"
    );
}

#[test]
fn installation_token_rejects_interior_whitespace() {
    let input = b"ghs_fake installation_token\n";
    let mut stdin = input.as_slice();
    let failure = read_installation_token(&mut stdin)
        .expect_err("whitespace inside a token would corrupt the header");

    assert_eq!(
        failure,
        super::CredentialError::StandardInput {
            reason: "must not contain whitespace",
        },
        "interior whitespace must be named as the reason"
    );
}

#[test]
fn installation_token_rejects_invalid_utf8() {
    let input = [0xff_u8, 0xfe];
    let mut stdin = input.as_slice();
    let failure = read_installation_token(&mut stdin)
        .expect_err("a non-UTF-8 credential cannot be sent as a header");

    assert_eq!(
        failure,
        super::CredentialError::StandardInput {
            reason: "not valid UTF-8",
        },
        "the rejection must name the encoding rather than the content"
    );
}

#[test]
fn installation_token_rejects_one_byte_over_the_limit() {
    let mut input = vec![b'a'; MAX_INSTALLATION_TOKEN_BYTES];
    input.push(b'\n');
    let mut stdin = input.as_slice();
    let failure =
        read_installation_token(&mut stdin).expect_err("one byte over the limit must be refused");

    assert_eq!(
        failure,
        super::CredentialError::StandardInput {
            reason: "exceeds 16 KiB",
        },
        "the limit must be reported as the input bound it crossed"
    );
}

#[test]
fn installation_token_accepts_exactly_the_limit_with_a_line_ending() {
    let mut input = vec![b'a'; MAX_INSTALLATION_TOKEN_BYTES - 1];
    input.push(b'\n');
    let mut stdin = input.as_slice();
    let token =
        read_installation_token(&mut stdin).expect("input exactly at the limit must be accepted");

    assert_eq!(
        token.secret().len(),
        MAX_INSTALLATION_TOKEN_BYTES - 1,
        "the accepted token is the input without its line ending"
    );
}

#[test]
fn observations_stop_at_the_first_failed_line() {
    let repositories = [
        repository(1, "octocat", "hello-world"),
        repository(2, "acme", "service"),
    ];
    let first_line = "octocat/hello-world\n";
    let mut writer = ByteBudgetWriter::new(first_line.len());

    let failure = write_observations(&mut writer, &repositories)
        .expect_err("a stream that rejects the second line must report failure");

    assert_eq!(
        failure.kind(),
        io::ErrorKind::Other,
        "the writer's own error must reach the caller unchanged"
    );
    assert_eq!(
        String::from_utf8(writer.written).expect("the accepted bytes are ASCII"),
        first_line,
        "only the lines written before the failure may be visible"
    );
}

/// Builds one validated summary for an output test.
fn repository(id: u64, owner: &str, name: &str) -> RepositorySummary {
    RepositoryFixture::public(id, owner, name)
        .build()
        .expect("a fixture with a valid identity should build")
}
