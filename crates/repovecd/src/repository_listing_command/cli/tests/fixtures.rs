//! Shared fixtures for the command-line unit tests.
//!
//! The harness runs `cli::run` over in-memory streams and a credential store
//! that never succeeds, so every case is decided without a process, a network
//! request, or a real credential. Typed case values keep the tests free of
//! stringly-typed argument lists.

use std::{
    cell::Cell,
    ffi::OsString,
    io::{self, Cursor, Write},
};

use camino::Utf8Path;
use repovec_core::github_oauth::AccessToken;

use super::super::{CliRun, run};
use crate::repository_listing_command::{CredentialError, UserTokenLoader};

/// Standard input holding nothing at all.
pub(super) const EMPTY_INPUT: &[u8] = &[];

/// Bytes that are not valid UTF-8.
const INVALID_UTF8: &[u8] = &[0xff, 0xfe];

/// A token carrying interior whitespace, which would corrupt a header.
const WHITESPACE_TOKEN: &[u8] = b"ghs_fake token\n";

/// A command line under test: the arguments after the program name.
pub(super) struct Arguments(Vec<OsString>);

impl Arguments {
    /// No arguments at all.
    pub(super) fn none() -> Self { Self(Vec::new()) }

    /// Both credential modes at once, which is never valid.
    fn both_modes() -> Self {
        Self(vec![
            OsString::from("--installation-token-stdin"),
            OsString::from("--user-store"),
            OsString::from("/etc/repovec"),
        ])
    }

    /// A literal token argument, which is never offered.
    fn literal_token() -> Self { Self(vec![OsString::from("--token"), OsString::from("literal")]) }

    /// A user-store mode with no directory after it.
    fn missing_store_directory() -> Self { Self(vec![OsString::from("--user-store")]) }

    /// The long form of the usage request.
    pub(super) fn long_help() -> Self { Self(vec![OsString::from("--help")]) }

    /// The short form of the usage request.
    pub(super) fn short_help() -> Self { Self(vec![OsString::from("-h")]) }

    /// One installation mode, which reads its token from standard input.
    pub(super) fn installation_stdin() -> Self {
        Self(vec![OsString::from("--installation-token-stdin")])
    }

    /// One user-store mode below the production directory.
    pub(super) fn user_store() -> Self {
        Self(vec![
            OsString::from("--user-store"),
            OsString::from("/etc/repovec"),
        ])
    }

    /// Builds a command line from arguments that are not text.
    #[cfg(unix)]
    pub(super) fn from_raw(arguments: Vec<OsString>) -> Self { Self(arguments) }
}

/// A command line the listing cannot act on, and its diagnostic.
#[derive(Clone, Copy, Debug)]
pub(super) enum Rejection {
    /// No credential mode was named.
    NoMode,
    /// More than one credential mode was named.
    TwoModes,
    /// An argument the surface does not offer.
    UnknownArgument,
    /// A user-store mode with no directory.
    MissingStoreDirectory,
}

impl Rejection {
    /// The command line that must be rejected.
    pub(super) fn arguments(self) -> Arguments {
        match self {
            Self::NoMode => Arguments::none(),
            Self::TwoModes => Arguments::both_modes(),
            Self::UnknownArgument => Arguments::literal_token(),
            Self::MissingStoreDirectory => Arguments::missing_store_directory(),
        }
    }

    /// Text the diagnostic must contain.
    pub(super) const fn fragment(self) -> &'static str {
        match self {
            Self::NoMode | Self::TwoModes => "exactly one credential mode is required",
            Self::UnknownArgument => "unknown argument",
            Self::MissingStoreDirectory => "--user-store requires a directory",
        }
    }
}

/// Standard input the credential reader must refuse.
#[derive(Clone, Copy, Debug)]
pub(super) enum UnusableInput {
    /// Nothing at all.
    Empty,
    /// Bytes that are not text.
    InvalidUtf8,
    /// A token with whitespace inside it.
    InteriorWhitespace,
}

impl UnusableInput {
    /// The bytes offered to standard input.
    pub(super) const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Empty => EMPTY_INPUT,
            Self::InvalidUtf8 => INVALID_UTF8,
            Self::InteriorWhitespace => WHITESPACE_TOKEN,
        }
    }

    /// Text the diagnostic must contain.
    pub(super) const fn fragment(self) -> &'static str {
        match self {
            Self::Empty => "standard input",
            Self::InvalidUtf8 => "not valid UTF-8",
            Self::InteriorWhitespace => "must not contain whitespace",
        }
    }
}

/// Which stream refuses every write.
#[derive(Clone, Copy, Debug)]
pub(super) enum RefusedStream {
    /// Standard output refuses.
    Stdout,
    /// Standard error refuses.
    Stderr,
}

/// A stream that either collects what is written or refuses every byte.
enum Sink {
    /// Collects the bytes it is offered.
    Collecting(Vec<u8>),
    /// Refuses every byte, as a closed pipe does.
    Refusing,
}

impl Sink {
    /// The collected bytes as text; a refusing sink collected nothing.
    fn text(&self) -> String {
        match self {
            Self::Collecting(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            Self::Refusing => String::new(),
        }
    }
}

impl Write for Sink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Collecting(bytes) => {
                bytes.extend_from_slice(buffer);
                Ok(buffer.len())
            }
            Self::Refusing => Err(io::Error::other("the stream is closed")),
        }
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

/// A loader that reports the user store as unconfigured and counts its calls.
#[derive(Default)]
struct CountingUserStore {
    /// How many times the command line asked for the user credential.
    calls: Cell<u32>,
}

impl UserTokenLoader for CountingUserStore {
    fn load_user_token(&self, _root: &Utf8Path) -> Result<AccessToken, CredentialError> {
        self.calls.set(self.calls.get().saturating_add(1));
        Err(CredentialError::UserStore {
            reason: "not configured".to_owned(),
        })
    }
}

/// What one command line produced.
pub(super) struct Outcome {
    /// The status the command line returned.
    pub(super) status: u8,
    /// Everything written to standard output.
    pub(super) stdout: String,
    /// Everything written to standard error.
    pub(super) stderr: String,
    /// How many times the credential store was consulted.
    pub(super) loader_calls: u32,
}

/// One command line run over in-memory streams and a fake credential store.
pub(super) struct Harness {
    /// The arguments after the program name.
    arguments: Vec<OsString>,
    /// Standard input offered to the command line.
    stdin: Cursor<Vec<u8>>,
    /// Stream the command line writes its output to.
    stdout: Sink,
    /// Stream the command line writes its diagnostics to.
    stderr: Sink,
    /// Credential store the command line consults.
    loader: CountingUserStore,
}

impl Harness {
    /// Builds a harness over the given arguments and standard input.
    pub(super) fn new(arguments: Arguments, input: &[u8]) -> Self {
        Self {
            arguments: arguments.0,
            stdin: Cursor::new(input.to_vec()),
            stdout: Sink::Collecting(Vec::new()),
            stderr: Sink::Collecting(Vec::new()),
            loader: CountingUserStore::default(),
        }
    }

    /// Builds a harness whose named stream refuses every write.
    pub(super) fn refusing(refused: RefusedStream, arguments: Arguments) -> Self {
        let mut harness = Self::new(arguments, EMPTY_INPUT);
        match refused {
            RefusedStream::Stdout => harness.stdout = Sink::Refusing,
            RefusedStream::Stderr => harness.stderr = Sink::Refusing,
        }
        harness
    }

    /// Runs the command line and collects its outcome.
    pub(super) fn run(mut self) -> Outcome {
        let status = {
            let mut command_line = CliRun {
                arguments: &self.arguments,
                stdin: &mut self.stdin,
                stdout: &mut self.stdout,
                stderr: &mut self.stderr,
                load_user_token: &self.loader,
            };
            run(&mut command_line)
        };
        Outcome {
            status,
            stdout: self.stdout.text(),
            stderr: self.stderr.text(),
            loader_calls: self.loader.calls.get(),
        }
    }
}

/// Runs one command line over the given arguments and standard input.
pub(super) fn run_command_line(arguments: Arguments, input: &[u8]) -> Outcome {
    Harness::new(arguments, input).run()
}

/// Runs one command line with empty standard input.
pub(super) fn run_without_input(arguments: Arguments) -> Outcome {
    run_command_line(arguments, EMPTY_INPUT)
}
