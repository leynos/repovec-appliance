"""Helpers for evaluating the development build standard's contracts.

The contracts assert what Make *evaluates* to, not what the Makefile
text contains. ``make --dry-run`` prints each recipe after variable
expansion without running it, so the lines read here carry the flags a
real invocation would use. Splitting those lines with ``shlex`` then
lets a contract assert a flag is present as an argument rather than as
a substring of a comment or a variable name.

The probes run against this repository's Makefile. Callers pass
``CARGO=probe-cargo`` so no line can be mistaken for a real Cargo run,
and ``BUILD_TOOLS_PREFIX=/x`` so the prefix in the probe line is
obviously not a developer's installation.
"""

from __future__ import annotations

import os
import platform
import re
import shlex
import subprocess
import tomllib
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

REPO_ROOT: Final = Path(__file__).resolve().parents[2]
MAKEFILE_PATH: Final = REPO_ROOT / "Makefile"
CARGO_CONFIG_PATH: Final = REPO_ROOT / ".cargo" / "config.toml"
MOLD_VERSION_PATH: Final = REPO_ROOT / "tools" / "mold" / "VERSION"
MOLD_SHA256SUMS_PATH: Final = REPO_ROOT / "tools" / "mold" / "SHA256SUMS"
WORKFLOW_PATH: Final = REPO_ROOT / ".github" / "workflows" / "ci.yml"

THREADS_FLAG: Final = "-Zthreads=8"
MOLD_FLAG: Final = "-Clink-arg=-fuse-ld=mold"
#: The warning denial as the two tokens Cargo receives. Keeping the
#: split form avoids comparing a token list against a two-word string.
WARNING_TOKENS: Final = ("-D", "warnings")

#: The Cargo executable the dry runs are told to use. A name that cannot
#: exist on PATH makes a leaked probe line unmistakable in a failure.
PROBE_CARGO: Final = "probe-cargo"

#: Bound on every child process the build-standard contract tests start.
#: Without it, a script that waits on a network, a lock, or a stuck toolchain
#: probe stalls the gate instead of failing with a diagnosis. The limit is
#: deliberately generous for a loaded CI runner; reaching it raises
#: ``subprocess.TimeoutExpired``, which fails the test with the command that
#: was still running.
SUBPROCESS_TIMEOUT_SECONDS: Final = 120

#: Set on every probe so the Makefile's standard flags are evaluated in
#: the same environment CI uses: ``setup-rust`` exports ``-D warnings``
#: for the whole job, which displaces ``.cargo/config.toml``.
CI_RUSTFLAGS: Final = "-D warnings"

#: Environment variables that would change how the nested Make behaves.
#: ``MAKEFLAGS`` carries the parent's jobserver and target list. The Make
#: control variables -- ``BUILD_JOBS`` and the per-target flag groups -- are
#: scrubbed as well, because the probes assert the Makefile's own defaults;
#: an inherited value would replace the flags under test and let the probe
#: check the caller's environment instead of the repository's standard.
_SCRUBBED_ENVIRONMENT: Final = (
    "MAKEFLAGS",
    "MFLAGS",
    "MAKELEVEL",
    "BUILD_JOBS",
    "CARGO_FLAGS",
    "TEST_FLAGS",
    "DOCTEST_FLAGS",
    "CLIPPY_FLAGS",
    "BASE_RUST_FLAGS",
    "RUST_FLAGS",
    "BASE_RUSTDOC_FLAGS",
    "RUSTDOC_FLAGS",
)

#: A leading ``NAME=value`` shell assignment. ``shlex`` keeps the whole
#: assignment, spaces in the value included, as one token.
_ASSIGNMENT_RE: Final = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=")

#: ``make`` invocations in a CI run block, optionally with short options.
_MAKE_RE: Final = re.compile(r"\bmake\s+(?:-[A-Za-z]+\s+)*([A-Za-z0-9_.-]+)")

#: ``cargo`` invocations in a CI run block.
_CARGO_RE: Final = re.compile(r"\bcargo\s+([a-z][a-z-]*)")

#: Cargo subcommands that compile or link workspace crates. ``fmt``,
#: ``metadata`` and ``install`` are deliberately absent: the first two do
#: not compile the workspace, and ``cargo install`` builds a tool rather
#: than the repository's own code.
COMPILING_CARGO_SUBCOMMANDS: Final = frozenset(
    {"build", "check", "clippy", "doc", "run", "rustc", "test", "nextest"}
)

#: Make targets whose recipes compile or link workspace crates. Each is a
#: prerequisite of ``check-build-tools`` in the Makefile, which the
#: contract also asserts separately.
COMPILING_MAKE_TARGETS: Final = frozenset(
    {
        "build",
        "build-docs-gate-helper",
        "test",
        "lint",
        "typecheck",
        "validate-systemd",
    }
)


@dataclass(frozen=True)
class Invocation:
    """One shell command line from a dry run or CI run block.

    Attributes
    ----------
    executable
        The command's name, after any leading ``NAME=value`` assignments.
    arguments
        The remaining words, excluding the executable.
    environment
        Leading ``NAME=value`` assignments, in file order.
    text
        The whole line, kept for failure messages.
    """

    executable: str
    arguments: tuple[str, ...]
    environment: dict[str, str]
    text: str

    def environment_flag_tokens(self, name: str) -> list[str]:
        """Return one environment value split into flag tokens.

        Examples
        --------
        >>> Invocation("cargo", (), {"RUSTFLAGS": "-D warnings"}, "").environment_flag_tokens("RUSTFLAGS")
        ['-D', 'warnings']
        """
        return shlex.split(self.environment.get(name, ""))


def is_linux() -> bool:
    """Report whether the host uses the Linux-only mold linker."""
    return platform.system() == "Linux"


def expected_standard_tokens() -> list[str]:
    """Return the development standard's flags for this host."""
    tokens = [THREADS_FLAG]
    if is_linux():
        tokens.append(MOLD_FLAG)
    return tokens


def _trailing_backslashes(text: str) -> int:
    """Count the backslashes that end ``text``."""
    return len(text) - len(text.rstrip("\\"))


def is_continued(line: str) -> bool:
    """Report whether ``line`` ends in a Make continuation.

    Make continues a recipe line only for an odd run of trailing
    backslashes: ``a \\`` continues, while ``a \\\\`` is a literal
    backslash at the end of the line and does not.
    """
    return _trailing_backslashes(line.rstrip()) % 2 == 1


def join_continuations(lines: list[str]) -> list[str]:
    """Join backslash-continued shell lines into one line each.

    ``make --dry-run`` prints a multi-line recipe with a trailing
    backslash on every line but the last, exactly as it would hand it to
    the shell. Joining first means a command is parsed whole rather than
    in fragments that each fail to parse.
    """
    joined: list[str] = []
    pending = ""
    for line in lines:
        stripped = line.rstrip()
        if is_continued(stripped):
            pending += stripped[:-1] + " "
            continue
        joined.append((pending + stripped).strip())
        pending = ""
    if pending:
        joined.append(pending.strip())
    return [line for line in joined if line]


_DOUBLE_QUOTE_ESCAPES: Final = frozenset('$`"\\\n')


def shell_tokens(line: str) -> list[str]:
    """Split one shell line into words the way ``sh`` does, failing on bad quoting.

    ``shlex`` in POSIX mode keeps the backslash before a dollar sign or a backtick
    inside double quotes, but a shell removes it. The recipes escape those two
    characters when they compose a double-quoted ``RUSTFLAGS``, so the reader
    must undo the escape for the contracts to see the caller's text.

    Examples
    --------
    >>> shell_tokens('RUSTFLAGS="-Cvar=\\$HOME a" cargo build')
    ['RUSTFLAGS=-Cvar=$HOME a', 'cargo', 'build']
    """
    words: list[str] = []
    word = ""
    started = False
    quote = ""
    index = 0
    while index < len(line):
        char = line[index]
        if quote == "'":
            if char == "'":
                quote = ""
            else:
                word += char
        elif quote == '"':
            if char == '"':
                quote = ""
            elif char == "\\" and line[index + 1 : index + 2] in _DOUBLE_QUOTE_ESCAPES:
                index += 1
                word += line[index]
            else:
                word += char
        elif char in "'\"":
            quote = char
            started = True
        elif char == "\\" and index + 1 < len(line):
            index += 1
            word += line[index]
            started = True
        elif char.isspace():
            if word or started:
                words.append(word)
            word = ""
            started = False
        else:
            word += char
            started = True
        index += 1
    if quote:
        message = f"unterminated {quote} quote in {line!r}"
        raise ValueError(message)
    if word or started:
        words.append(word)
    return words


def invocation(line: str) -> Invocation:
    """Parse one shell line into its executable, arguments and environment.

    The leading assignments are the ones that matter to these contracts:
    ``RUSTFLAGS=... cargo build`` sets the flags for that command alone.

    Raises
    ------
    ValueError
        When the line contains no command after its assignments.
    """
    tokens = shell_tokens(line)
    environment: dict[str, str] = {}
    index = 0
    while index < len(tokens):
        match = _ASSIGNMENT_RE.match(tokens[index])
        if not match:
            break
        environment[match.group(1)] = tokens[index][match.end() :]
        index += 1
    if index == len(tokens):
        message = f"no command in shell line: {line!r}"
        raise ValueError(message)
    return Invocation(tokens[index], tuple(tokens[index + 1 :]), environment, line)


def run_make_dry_run(
    target: str,
    *,
    rustflags: str | None = CI_RUSTFLAGS,
    extra_environment: dict[str, str] | None = None,
) -> list[str]:
    """Return the evaluated recipe lines for one Make target.

    Parameters
    ----------
    target
        The Make target to dry-run.
    rustflags
        Value assigned to ``RUSTFLAGS`` for the probe, or ``None`` to
        remove it as an interactive shell usually has it unset.
    extra_environment
        Further environment overrides, applied last.

    Returns
    -------
    list[str]
        The dry run's output lines, continuations joined.

    Raises
    ------
    AssertionError
        When Make rejects the invocation.
    """
    environment = os.environ.copy()
    for name in _SCRUBBED_ENVIRONMENT:
        environment.pop(name, None)
    if rustflags is None:
        environment.pop("RUSTFLAGS", None)
    else:
        environment["RUSTFLAGS"] = rustflags
    if extra_environment:
        environment.update(extra_environment)

    completed = subprocess.run(
        [
            "make",
            "--no-print-directory",
            "--dry-run",
            target,
            f"CARGO={PROBE_CARGO}",
            "BUILD_TOOLS_PREFIX=/x",
        ],
        cwd=REPO_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )
    assert completed.returncode == 0, (
        f"make --dry-run {target} failed with {completed.returncode}:\n"
        f"{completed.stdout}\n{completed.stderr}"
    )
    return join_continuations(completed.stdout.splitlines())


def cargo_invocations(lines: list[str]) -> list[Invocation]:
    """Return the lines that invoke the probe Cargo executable.

    A mention of the probe name inside ``command -v`` or ``test -x`` is
    not an invocation; only a line whose command word is the probe
    executable counts.
    """
    found: list[Invocation] = []
    for line in lines:
        parsed = invocation(line)
        if parsed.executable == PROBE_CARGO:
            found.append(parsed)
    return found


def is_runner_invocation(invocation_: Invocation) -> bool:
    """Report whether a command runs the Rust test-gate runner itself.

    The ``script-test`` prerequisite runs ``ruff`` over the runner's
    source, so the path also appears as an argument to ``uv tool run``.
    A real runner invocation is not a ``uv tool`` command: it is
    ``uv run ... scripts/run_rust_tests.py``.
    """
    if "tool" in invocation_.arguments:
        return False
    return any(
        argument == "scripts/run_rust_tests.py" for argument in invocation_.arguments
    )


def runner_invocations(lines: list[str]) -> list[Invocation]:
    """Return the lines that invoke the Rust test-gate runner."""
    return [
        parsed
        for parsed in (invocation(line) for line in lines)
        if is_runner_invocation(parsed)
    ]


def option_value(invocation_: Invocation, name: str) -> str | None:
    """Return a ``--name=value`` option's value, or ``None``.

    The runner's recipe writes every option in the ``--name=value``
    form, which is one shell word and therefore one token.
    """
    prefix = f"--{name}="
    for argument in invocation_.arguments:
        if argument.startswith(prefix):
            return argument[len(prefix) :]
    return None


def first_compiling_index(lines: list[str]) -> int:
    """Return the index of the first line that compiles workspace code.

    For the ``test`` route the compilation is inside the runner, so the
    runner's own line is the compiling step. Elsewhere it is a direct
    Cargo invocation.
    """
    for index, line in enumerate(lines):
        parsed = invocation(line)
        if parsed.executable == PROBE_CARGO:
            return index
        if is_runner_invocation(parsed):
            return index
    raise AssertionError(f"no compiling line in dry run:\n{chr(10).join(lines)}")


def preflight_index(lines: list[str]) -> int | None:
    """Return the index of the build-tools preflight, or ``None``."""
    for index, line in enumerate(lines):
        if "check-build-tools.sh" in line:
            return index
    return None


def load_cargo_config() -> dict[str, object]:
    """Parse ``.cargo/config.toml``."""
    return tomllib.loads(CARGO_CONFIG_PATH.read_text(encoding="utf-8"))


def iter_keys(value: object, path: tuple[object, ...] = ()) -> list[tuple[object, ...]]:
    """Return every mapping key in a parsed TOML document with its path."""
    if isinstance(value, dict):
        return _descend(value.items(), path, is_keyed=True)
    if isinstance(value, list):
        return _descend(enumerate(value), path, is_keyed=False)
    return []


def _descend(
    pairs: Iterable[tuple[object, object]],
    path: tuple[object, ...],
    *,
    is_keyed: bool,
) -> list[tuple[object, ...]]:
    """Return each child's key, when keyed, followed by its nested keys."""
    found: list[tuple[object, ...]] = []
    for component, child in pairs:
        child_path = (*path, component)
        declared = (child_path,) if is_keyed else ()
        found += declared + tuple(iter_keys(child, child_path))
    return found


def iter_strings(value: object) -> list[str]:
    """Return every string value in a parsed TOML document."""
    if isinstance(value, dict):
        return _descend_strings(value.values())
    if isinstance(value, list):
        return _descend_strings(value)
    return [value] if isinstance(value, str) else []


def _descend_strings(children: Iterable[object]) -> list[str]:
    """Return every string nested inside the given children."""
    return [found for child in children for found in iter_strings(child)]


def run_block_lines(run_value: object) -> list[str]:
    """Return the non-blank, non-comment lines of a CI ``run`` value."""
    if not isinstance(run_value, str):
        return []
    return [
        line.strip()
        for line in run_value.splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]


def make_targets(run_value: object) -> list[str]:
    """Return the Make targets a CI ``run`` value invokes."""
    return [
        match.group(1)
        for line in run_block_lines(run_value)
        for match in _MAKE_RE.finditer(line)
    ]


def cargo_subcommands(run_value: object) -> list[str]:
    """Return the Cargo subcommands a CI ``run`` value invokes."""
    return [
        match.group(1)
        for line in run_block_lines(run_value)
        for match in _CARGO_RE.finditer(line)
    ]


def run_value_compiles(run_value: object) -> bool:
    """Report whether a CI ``run`` value compiles or links Rust.

    Examples
    --------
    >>> run_value_compiles("make build")
    True
    >>> run_value_compiles("make spelling")
    False
    """
    if any(target in COMPILING_MAKE_TARGETS for target in make_targets(run_value)):
        return True
    return any(
        subcommand in COMPILING_CARGO_SUBCOMMANDS
        for subcommand in cargo_subcommands(run_value)
    )
