//! Execute systemd credential transforms and interpret command failures.

use camino::Utf8Path;

use super::{CommandRunner, LossyStderr, SystemdCredsError};

/// Borrows all inputs needed for one systemd credential transformation.
pub(super) struct SystemdCredsTransform<'a, R>
where
    R: CommandRunner,
{
    /// Runner used to execute the command and capture its output.
    pub(super) runner: &'a R,
    /// Path of the systemd credential transformation executable.
    pub(super) command: &'a Utf8Path,
    /// Transformation verb, such as `encrypt` or `decrypt`.
    pub(super) operation: &'static str,
    /// Optional credential identity passed through the command name argument.
    pub(super) name: Option<&'a str>,
    /// Credential bytes supplied through standard input rather than command arguments.
    pub(super) input: &'a [u8],
}

/// Runs a credential transformation and returns stdout only on a zero exit status.
/// For example, `encrypt` passes bytes through stdin; a failed process returns its error.
pub(super) fn run_systemd_creds_file_transform<R>(
    transform: &SystemdCredsTransform<'_, R>,
) -> Result<Vec<u8>, SystemdCredsError<R::Error>>
where
    R: CommandRunner,
{
    let mut args = vec![transform.operation.to_owned()];
    if let Some(credential_name) = transform.name {
        args.push(format!("--name={credential_name}"));
    }
    args.push("-".to_owned());
    args.push("-".to_owned());

    let output = transform
        .runner
        .run(transform.command, args, transform.input)
        .map_err(SystemdCredsError::Run)?;
    if output.status_code != Some(0) {
        return Err(SystemdCredsError::CommandFailed {
            operation: transform.operation,
            status: output.status_code,
            stderr: LossyStderr(String::from_utf8_lossy(&output.stderr).into_owned()),
        });
    }

    Ok(output.stdout)
}
