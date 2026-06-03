"""Unit tests for :mod:`lib.result`.

These exercise the rendering branches of :class:`CommandResult` so
behaviour is pinned at the value-object level without dragging in
container or cuprum infrastructure. They run as part of the default
pytest collection — no markers required.
"""

from __future__ import annotations

import pytest

from lib.result import CommandResult


def _result(
    *,
    argv: tuple[str, ...] = ("echo", "hi"),
    exit_code: int = 0,
    stdout: str = "",
    stderr: str = "",
    cwd: str | None = None,
) -> CommandResult:
    return CommandResult(
        argv=argv,
        exit_code=exit_code,
        stdout=stdout,
        stderr=stderr,
        cwd=cwd,
    )


def test_ok_is_true_for_zero_exit() -> None:
    assert _result(exit_code=0).ok is True


def test_ok_is_false_for_non_zero_exit() -> None:
    assert _result(exit_code=1).ok is False


def test_render_emits_command_and_exit_code_only_for_minimal_result() -> None:
    rendered = _result().render()

    assert rendered == "command: echo hi\nexit_code: 0"


def test_render_quotes_argv_with_spaces() -> None:
    rendered = _result(argv=("/bin/sh", "-c", "echo 'one two'")).render()

    assert rendered.startswith("command: /bin/sh -c 'echo '\"'\"'one two'\"'\"''")


def test_render_quotes_argv_with_shell_metacharacters() -> None:
    rendered = _result(argv=("printf", "$X", ">file")).render()

    # ``shlex.join`` must quote both tokens so they do not look like a
    # variable expansion or a redirection if pasted into a shell.
    assert "'$X'" in rendered
    assert "'>file'" in rendered


def test_render_omits_cwd_when_none() -> None:
    rendered = _result(cwd=None).render()

    assert "cwd:" not in rendered


def test_render_includes_cwd_when_set() -> None:
    rendered = _result(cwd="/tmp").render()

    assert "cwd: /tmp" in rendered


def test_render_appends_stdout_block_only_when_present() -> None:
    rendered = _result(stdout="line1\nline2\n").render()

    assert rendered.endswith("stdout:\nline1\nline2")
    assert "stderr:" not in rendered


def test_render_trims_trailing_whitespace_from_stream_blocks() -> None:
    rendered = _result(stdout="payload\n\n\n").render()

    # ``rstrip()`` strips trailing whitespace; the block should not carry
    # a runaway trail of newlines into the diagnostic.
    assert rendered.endswith("stdout:\npayload")


def test_render_appends_stderr_block_when_present() -> None:
    rendered = _result(stderr="oops\n").render()

    assert rendered.endswith("stderr:\noops")
    assert "stdout:" not in rendered


def test_render_orders_command_cwd_exit_stdout_stderr() -> None:
    rendered = _result(
        argv=("ls",),
        exit_code=2,
        stdout="files\n",
        stderr="warning\n",
        cwd="/var",
    ).render()

    lines = rendered.split("\n")
    assert lines[0] == "command: ls"
    assert lines[1] == "cwd: /var"
    assert lines[2] == "exit_code: 2"
    # The remaining lines are the stdout block followed by the stderr block.
    assert lines.index("stdout:") < lines.index("stderr:")


def test_result_is_frozen() -> None:
    result = _result()
    with pytest.raises(dataclasses_frozen_error()):
        result.exit_code = 99  # type: ignore[misc]


def dataclasses_frozen_error() -> type[Exception]:
    """Return the exception type raised when mutating a frozen dataclass.

    ``dataclasses`` raises ``FrozenInstanceError`` (a subclass of
    ``AttributeError``); the indirection keeps the test source readable
    without an extra import in every file that needs the type.
    """

    import dataclasses

    return dataclasses.FrozenInstanceError
