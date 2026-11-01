"""Hold where the publisher may name the CodeScene token (CV-005).

The uploader is a composite action, so a token bound in the upload step's
``env`` would be handed to the action's nested steps as well. The action reads
the token from ``inputs.access-token``, and the publisher therefore passes it
there and nowhere else.

The publisher cannot gate the upload on ``env.CS_ACCESS_TOKEN``: deleting the
binding would make that expression false rather than failing anything, and the
upload would skip in silence for the life of the repository. The presence check
is a step of its own, with one exact command that Actions expands before the
shell runs. This module holds that step, its position, and the token's scope.
"""

from __future__ import annotations

import typing as typ

from coverage_workflow_reader import (
    Document,
    Step,
    continues_on_error,
    folded,
    holding_job,
    jobs,
    scalars,
)

#: The id the upload guard reads.
CHECK_ID: typ.Final[str] = "codescene-token"

#: The presence check's whole command. Actions evaluates the expression to
#: ``true`` or ``false`` before the shell runs, so the step holds no shell
#: conditional and binds the token in no ``env``. A run without the secret
#: writes ``available=false`` and skips the upload instead of failing.
CHECK_COMMAND: typ.Final[str] = (
    'echo "available=${{ secrets.CS_ACCESS_TOKEN != \'\' }}"'
    ' >> "$GITHUB_OUTPUT"'
)

#: Where the upload receives the token.
CREDENTIAL_INPUT: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"

#: The only keys the presence check may carry. ``if``, ``uses``, ``shell``,
#: ``env`` or ``continue-on-error`` would skip it, run other code, bind the
#: token or turn its failure green.
CHECK_KEYS: typ.Final[frozenset[str]] = frozenset({"name", "id", "run"})


def expression(value: object) -> str:
    """Return an expression with its inner whitespace normalized.

    Parameters
    ----------
    value
        A scalar read from a parsed workflow.

    Returns
    -------
    str
        The text with ``${{`` and ``}}`` spaced and runs of whitespace
        collapsed to a single space.

    Examples
    --------
    >>> expression("${{secrets.X}}")
    '${{ secrets.X }}'
    """
    spaced = str(value).replace("${{", "${{ ").replace("}}", " }}")
    return " ".join(spaced.split())


def _check_problems(
    name: str, check: Step, upload_index: int, check_index: int
) -> list[str]:
    """Report a presence check that could skip, fail green or run other code."""
    conditions = (
        ("must run before the upload", check_index > upload_index),
        (f"may carry only {sorted(CHECK_KEYS)}", not check.keys() <= CHECK_KEYS),
        ("must not continue on error", continues_on_error(check)),
        (
            f"must run exactly `{CHECK_COMMAND}`",
            expression(check.get("run", "")) != CHECK_COMMAND,
        ),
    )
    return [
        f"{name} `{CHECK_ID}` step {problem}"
        for problem, failed in conditions
        if failed
    ]


def _names_token(value: object) -> bool:
    """Return whether any scalar in the value names the CodeScene token."""
    return any("cs_access_token" in folded(text) for text in scalars(value))


def _job_scope(job: Step) -> Step:
    """Return a job's fields other than the steps it runs."""
    return {str(key): value for key, value in job.items() if key != "steps"}


def _unkept_steps(job: Step, kept: list[Step]) -> list[Step]:
    """Return a job's steps other than the ones allowed to hold the token."""
    return [
        step
        for step in typ.cast("list[Step]", job.get("steps", []))
        if not any(step is held for held in kept)
    ]


def _outside_the_two_steps(
    name: str, document: Document, kept: list[Step]
) -> list[str]:
    """Report the token anywhere in the publisher but the two named steps.

    Job- and workflow-scope scalars are read as well as the other steps, so a
    token in a job's ``env`` or a workflow's ``defaults`` is seen.
    """
    publisher = list(jobs(name, document).values())
    scopes: list[object] = [_job_scope(job) for job in publisher]
    for job in publisher:
        scopes.extend(_unkept_steps(job, kept))
    scopes.append({key: value for key, value in document.items() if key != "jobs"})
    if any(_names_token(scope) for scope in scopes):
        return [f"{name} puts CS_ACCESS_TOKEN in reach outside its two steps"]
    return []


def _upload_env(name: str, upload: Step) -> list[str]:
    """Report a token in the upload step's ``env``, which nested steps inherit."""
    if any("cs_access_token" in folded(text) for text in scalars(upload.get("env"))):
        return [f"{name} upload step must not bind CS_ACCESS_TOKEN in its env"]
    return []


def token_violations(name: str, document: Document, upload: Step) -> list[str]:
    """Report a token placed anywhere but the check command and the upload input.

    Parameters
    ----------
    name
        The publisher's file name, for messages.
    document
        The parsed publisher.
    upload
        The publisher's upload step.

    Returns
    -------
    list[str]
        One message per violation; empty when the token is placed as required.

    Raises
    ------
    StopIteration
        If the upload step does not belong to any job in the document.
    """
    held = typ.cast(
        "list[Step]", holding_job(name, document, upload).get("steps", [])
    )
    checks = [index for index, step in enumerate(held) if step.get("id") == CHECK_ID]
    if len(checks) != 1:
        return [f"{name} needs one `{CHECK_ID}` step in the upload job"]
    check = held[checks[0]]
    inputs = upload.get("with")
    inputs = inputs if isinstance(inputs, dict) else {}
    problems = _check_problems(name, check, held.index(upload), checks[0])
    if expression(inputs.get("access-token")) != CREDENTIAL_INPUT:
        problems.append(f"{name} upload must pass access-token {CREDENTIAL_INPUT}")
    return [
        *problems,
        *_upload_env(name, upload),
        *_outside_the_two_steps(name, document, [check, upload]),
    ]
