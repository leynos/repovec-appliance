//! Execute systemd credential transforms and interpret command failures.

use camino::Utf8Path;

use super::{CommandRunner, LossyStderr, SystemdCredsError};

pub(super) struct SystemdCredsTransform<'a, R>
where
    R: CommandRunner,
{
    pub(super) runner: &'a R,
    pub(super) command: &'a Utf8Path,
    pub(super) operation: &'static str,
    pub(super) name: Option<&'a str>,
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
