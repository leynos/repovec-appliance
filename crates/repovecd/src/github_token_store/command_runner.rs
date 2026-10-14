//! Execute credential commands with piped input and captured process output.

use std::{
    ffi::OsStr,
    io::Write,
    process::{Command, Stdio},
};

use camino::Utf8Path;

use super::{CommandOutput, CommandRunner, SystemCommandRunner};

impl CommandRunner for SystemCommandRunner {
    type Error = std::io::Error;

    fn run<I, S>(
        &self,
        command: &Utf8Path,
        args: I,
        stdin: &[u8],
    ) -> Result<CommandOutput, Self::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut child = Command::new(command.as_str())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut child_stdin) = child.stdin.take()
            && let Err(error) = child_stdin.write_all(stdin)
        {
            drop(child.kill());
            drop(child.wait());
            return Err(error);
        }
        let output = child.wait_with_output()?;
        Ok(CommandOutput {
            status_code: output.status.code(),
            stderr: output.stderr,
            stdout: output.stdout,
        })
    }
}
