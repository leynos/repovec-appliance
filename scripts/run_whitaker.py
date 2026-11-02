#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts==4.25.0", "plumbum==2.0.2"]
# ///
"""Run the Whitaker Dylint suite as one fail-closed gate.

The lint route used to be a Make recipe that asked ``command -v`` whether
Whitaker existed and printed a skip when it did not. A missing gate was
then indistinguishable from a passing one, so the suite was not binding.
This script resolves the executable, fails with an actionable message when
it is absent, and propagates the suite's own exit status. Those decisions
live here rather than in a recipe because a recipe cannot express them
without either a shell condition that can silently skip or a chain whose
status is discarded; here they are unit-tested.

The invocation is fixed to the reviewed route: ``--all`` loads every
discovered lint library and ``--workspace`` checks every workspace
package. The caller's Cargo flags follow the option terminator unchanged,
so the Makefile owns the flag policy and this script owns only the
failure semantics.
"""

from __future__ import annotations

import shlex
import shutil
import sys
from dataclasses import dataclass

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.commands.base import BaseCommand

app = App(config=cyclopts.config.Env("INPUT_", command=False))

MISSING_EXECUTABLE = (
    "error: Whitaker executable {whitaker!r} was not found or is not "
    "executable. Install the Whitaker Dylint suite (see "
    'docs/developers-guide.md, "Lint routes: Clippy and Whitaker") and '
    "set WHITAKER to its path."
)


@Parameter(name="*")
@dataclass(frozen=True)
class WhitakerOptions:
    """Configuration the Make recipe supplies, one field per flag.

    Attributes
    ----------
    whitaker
        Whitaker executable, by name or absolute path.
    packages
        Package-selection arguments for Cargo, as one shell-quoted
        string.
    cargo_flags
        Cargo check flags, as one shell-quoted string.
    """

    whitaker: str = "whitaker"
    packages: str = "--workspace"
    cargo_flags: str = "--all-targets --all-features"


def split_flags(flags: str | None) -> list[str]:
    """Split a Make-supplied flag string into arguments.

    An unset variable arrives as the empty string, which must contribute
    no argument rather than an empty one.
    """
    return shlex.split(flags or "")


def build_argv(*, packages: list[str], cargo_flags: list[str]) -> list[str]:
    """Return the Whitaker arguments for the reviewed lint route.

    ``--all`` selects every discovered lint library, the package
    selection checks every workspace package, and the Cargo flags are
    forwarded without reordering or rewriting.
    """
    return ["--all", *packages, "--", *cargo_flags]


def resolve_executable(whitaker: str) -> str | None:
    """Return the executable path for *whitaker*, or ``None`` if absent."""
    return shutil.which(whitaker)


def run_suite(command: BaseCommand, argv: list[str]) -> int:
    """Run Whitaker with its output inherited and return its exit status."""
    code, _, _ = command[argv].run(retcode=None, stdout=None, stderr=None)
    return code


@app.default
def main(*, options: WhitakerOptions = WhitakerOptions()) -> None:
    """Run the suite, failing closed when Whitaker is unavailable.

    Parameters
    ----------
    options
        Runner configuration. ``Parameter(name="*")`` flattens it, so
        each field is still spelled as its own command-line flag.
    """
    executable = resolve_executable(options.whitaker)
    if executable is None:
        print(
            MISSING_EXECUTABLE.format(whitaker=options.whitaker),
            file=sys.stderr,
        )
        sys.exit(1)

    argv = build_argv(
        packages=split_flags(options.packages),
        cargo_flags=split_flags(options.cargo_flags),
    )
    code = run_suite(local[executable], argv)
    if code != 0:
        sys.exit(code)


if __name__ == "__main__":
    app()
