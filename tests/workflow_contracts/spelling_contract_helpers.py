"""Run the consumer's spelling Make boundary against isolated tracked inputs.

The real builder's first run refreshes its live dictionary. Precision cases
reuse that observed dictionary offline so an upstream edit during one test run
does not become a spurious difference between cases. UV may resolve the pinned
builder release online; production remains live.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = ROOT / "Makefile"
BUILDER_SOURCE = "git+https://github.com/leynos/typos-config-builder.git@v0.1.3"
CACHE_FILES = (".typos-oxendict-base.toml", ".typos-oxendict-base.json")


def child_environment(**values: str) -> dict[str, str]:
    """Return explicit child settings without changing the parent environment."""
    environment = os.environ.copy()
    for key in tuple(environment):
        if key.startswith("GIT_CONFIG_") or key in {
            "GH_TOKEN", "GITHUB_TOKEN", "BASH_ENV", "GIT_SSH", "GIT_SSH_COMMAND"
        }:
            environment.pop(key)
    lody = str(Path.home() / ".lody")
    environment["PATH"] = os.pathsep.join(
        part for part in environment.get("PATH", "").split(os.pathsep)
        if part != lody and not part.startswith(lody + os.sep)
    )
    environment["GIT_SSH_COMMAND"] = "/usr/bin/ssh"
    environment.update(values)
    return environment


def spelling_tool_environment(directory: Path) -> dict[str, str]:
    """Resolve the pinned release online while sharing the measured tool cache."""
    return child_environment(
        UV_CACHE_DIR=str(directory / ".uv-cache"),
        UV_TOOL_DIR=str(directory / ".uv-tools"),
        UV_OFFLINE="0",
    )


def initialize_probe(directory: Path, filename: str, text: str) -> None:
    """Create an isolated Git repository with one non-ignored tracked input."""
    directory.mkdir()
    shutil.copyfile(ROOT / "typos.local.toml", directory / "typos.local.toml")
    probe = directory / filename
    probe.parent.mkdir(parents=True, exist_ok=True)
    probe.write_text(text, encoding="utf-8")
    environment = child_environment()
    subprocess.run(["git", "init", "-q", str(directory)], env=environment, check=True)
    ignored = subprocess.run(
        ["git", "-C", str(directory), "check-ignore", "--quiet", filename],
        env=environment, check=False,
    )
    if ignored.returncode != 1:
        raise RuntimeError(f"Probe input was ignored or Git failed: {filename}")
    subprocess.run(
        ["git", "-C", str(directory), "add", "--", filename],
        env=environment, check=True,
    )


def run_spelling(
    directory: Path,
    *,
    makefile: Path = MAKEFILE,
    overrides: tuple[str, ...] = (),
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Evaluate the real Make target and capture its failure-preserving result."""
    return subprocess.run(
        ["make", "--no-print-directory", "-f", str(makefile), "spelling", *overrides],
        cwd=directory, env=environment or child_environment(),
        text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
    )


def write_recorder(directory: Path) -> Path:
    """Supply the injectable UV seam with an argv recorder and controlled exit."""
    recorder = directory / "record-uv"
    recorder.write_text(
        f"#!{sys.executable}\n"
        "import json, os, pathlib, sys\n"
        "pathlib.Path(os.environ['SPELLING_RECORD']).write_text(json.dumps(sys.argv[1:]))\n"
        "raise SystemExit(int(os.environ['SPELLING_EXIT']))\n",
        encoding="utf-8",
    )
    recorder.chmod(0o755)
    return recorder


def recorded_arguments(
    directory: Path, *, makefile: Path = MAKEFILE, exit_code: int = 0,
) -> tuple[list[str], subprocess.CompletedProcess[str]]:
    """Return evaluated builder arguments and the enclosing Make result."""
    recorder = write_recorder(directory)
    record = directory / "arguments.json"
    result = run_spelling(
        directory, makefile=makefile, overrides=(f"UV={recorder}",),
        environment=child_environment(SPELLING_RECORD=str(record), SPELLING_EXIT=str(exit_code)),
    )
    return json.loads(record.read_text(encoding="utf-8")), result


def write_offline_builder(directory: Path) -> Path:
    """Reuse the measured dictionary without changing the Make target's argv."""
    wrapper = directory / "builder-offline"
    wrapper.write_text(
        f"#!{sys.executable}\n"
        "import os, subprocess, sys\n"
        f"command = ['uv', 'tool', 'run', '--python', '3.14', '--from', {BUILDER_SOURCE!r}, "
        "'typos-config-builder', *sys.argv[1:], '--offline']\n"
        "print('spelling-uv-cache=' + os.environ['UV_CACHE_DIR'], flush=True)\n"
        "print('spelling-uv-tools=' + os.environ['UV_TOOL_DIR'], flush=True)\n"
        "print('spelling-uv-offline=' + os.environ['UV_OFFLINE'], flush=True)\n"
        "status = subprocess.run(command).returncode\n"
        "print(f'spelling-builder-exit={status}', flush=True)\n"
        "raise SystemExit(status)\n",
        encoding="utf-8",
    )
    wrapper.chmod(0o755)
    return wrapper


def input_hashes(directory: Path) -> dict[str, str]:
    """Record observed live inputs so a new shared input is distinguishable."""
    return {
        filename: hashlib.sha256((directory / filename).read_bytes()).hexdigest()
        for filename in (*CACHE_FILES, "typos.local.toml")
    }
