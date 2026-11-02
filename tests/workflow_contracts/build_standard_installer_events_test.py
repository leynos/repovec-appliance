"""Contract tests for the installer's observable events.

The installer emits one bounded line per attempt and outcome so a failed
download or toolchain install can be diagnosed from a CI log without guessing
how many retries ran. These tests drive the real script with the shared
hermetic fixtures and assert the event stream.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from build_standard_installer_support import (
    MOLD_VERSION,
    TOOLCHAIN,
    Installer,
    installer,
    write_stub,
)

__all__ = ["installer"]

EVENT_PREFIX = "build-tools: event="
FULL_URL = (
    f"https://mold.example.invalid/releases/v{MOLD_VERSION}/"
    f"mold-{MOLD_VERSION}-x86_64-linux.tar.gz"
)


def events(stderr: str) -> list[dict[str, str]]:
    """Parse the installer's event lines into a name and its fields."""
    parsed: list[dict[str, str]] = []
    for line in stderr.splitlines():
        if not line.startswith(EVENT_PREFIX):
            continue
        parts = line[len(EVENT_PREFIX) :].split()
        fields = dict(part.split("=", 1) for part in parts[1:])
        parsed.append({"event": parts[0], **fields})
    return parsed


def named(found: list[dict[str, str]], name: str) -> list[dict[str, str]]:
    """Return the events with the given name."""
    return [event for event in found if event["event"] == name]


def assert_no_full_url(stderr: str) -> None:
    """Assert no event line carries the complete download URL."""
    for line in stderr.splitlines():
        if line.startswith(EVENT_PREFIX):
            assert FULL_URL not in line, line


def test_success_emits_one_attempt_and_outcome_per_boundary(
    installer: Installer,
) -> None:
    """Scenario: both install boundaries succeed.

    Invariant: the download and the toolchain install each emit one attempt
    and one success outcome, the download event names the host but not the
    full URL, and every outcome carries an elapsed time.
    """
    completed = installer.run()

    assert completed.returncode == 0, completed.stderr
    found = events(completed.stderr)

    attempts = named(found, "mold_download_attempt")
    outcomes = named(found, "mold_download_outcome")
    assert len(attempts) == 1, found
    assert attempts[0]["attempt"] == "1"
    assert attempts[0]["version"] == MOLD_VERSION
    assert attempts[0]["arch"] == "x86_64"
    assert attempts[0]["url_host"] == "mold.example.invalid"
    assert len(outcomes) == 1, found
    assert outcomes[0]["outcome"] == "success"
    assert "elapsed_ms" in outcomes[0]

    toolchain_attempts = named(found, "toolchain_install_attempt")
    toolchain_outcomes = named(found, "toolchain_install_outcome")
    assert len(toolchain_attempts) == 1, found
    assert toolchain_attempts[0]["toolchain"] == TOOLCHAIN
    assert len(toolchain_outcomes) == 1, found
    assert toolchain_outcomes[0]["outcome"] == "success"
    assert "elapsed_ms" in toolchain_outcomes[0]

    assert len(named(found, "mold_extract_outcome")) == 1, found
    install_outcomes = named(found, "mold_install_outcome")
    assert len(install_outcomes) == 1, found
    assert install_outcomes[0]["outcome"] == "success"
    assert_no_full_url(completed.stderr)


def test_a_transient_failure_is_retried_and_observable(
    installer: Installer,
) -> None:
    """Scenario: curl fails twice and then succeeds.

    Invariant: three attempts are visible, the first two report a failure
    with a category, and the third reports success. The bounded retry loop
    must not hide the failed attempts behind curl's own retry flags.
    """
    completed = installer.run(
        {
            "CURL_RETRIES": "2",
            "CURL_RETRY_MAX_SECONDS": "60",
            "STUB_CURL_FAIL_TIMES": "2",
        }
    )

    assert completed.returncode == 0, completed.stderr
    found = events(completed.stderr)
    attempts = named(found, "mold_download_attempt")
    outcomes = named(found, "mold_download_outcome")
    assert [event["attempt"] for event in attempts] == ["1", "2", "3"], found
    failures = [event for event in outcomes if event["outcome"] == "failure"]
    successes = [event for event in outcomes if event["outcome"] == "success"]
    assert len(failures) == 2, found
    assert len(successes) == 1, found
    assert successes[0]["attempt"] == "3"
    assert all("failure_category" in event for event in failures)
    assert all("elapsed_ms" in event for event in outcomes)
    assert_no_full_url(completed.stderr)


def test_a_permanent_failure_is_bounded(installer: Installer) -> None:
    """Scenario: every download attempt fails.

    Invariant: the installer makes exactly ``CURL_RETRIES + 1`` attempts,
    reports a failure outcome for each, and emits one failed install
    outcome. A permanent failure must not become an endless wait.
    """
    completed = installer.run(
        {
            "CURL_RETRIES": "2",
            "CURL_RETRY_MAX_SECONDS": "60",
            "STUB_CURL_FAIL_TIMES": "99",
        }
    )

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert "failed to download" in completed.stderr
    found = events(completed.stderr)
    attempts = named(found, "mold_download_attempt")
    outcomes = named(found, "mold_download_outcome")
    assert len(attempts) == 3, found
    assert len(outcomes) == 3, found
    assert all(event["outcome"] == "failure" for event in outcomes)
    assert all("failure_category" in event for event in outcomes)
    install_outcomes = named(found, "mold_install_outcome")
    assert len(install_outcomes) == 1, found
    assert install_outcomes[0]["outcome"] == "failure"
    assert_no_full_url(completed.stderr)


def test_a_failing_toolchain_install_emits_events(installer: Installer) -> None:
    """Scenario: rustup is present but the install command fails.

    Invariant: the toolchain boundary emits one attempt and one failure
    outcome with a category and an elapsed time.
    """
    write_stub(installer.root / "stub" / "rustup", "exit 1\n")

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert "failed to install toolchain" in completed.stderr
    found = events(completed.stderr)
    attempts = named(found, "toolchain_install_attempt")
    outcomes = named(found, "toolchain_install_outcome")
    assert len(attempts) == 1, found
    assert len(outcomes) == 1, found
    assert outcomes[0]["outcome"] == "failure"
    assert "failure_category" in outcomes[0]
    assert "elapsed_ms" in outcomes[0]


def test_a_missing_rustup_emits_a_failure_outcome(installer: Installer) -> None:
    """Scenario: rustup is absent from PATH.

    Invariant: the toolchain boundary still emits an attempt and a failure
    outcome, so the missing tool is visible in the event stream as well as
    in the human-facing diagnostic.
    """
    (installer.root / "stub" / "rustup").unlink()

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    found = events(completed.stderr)
    attempts = named(found, "toolchain_install_attempt")
    outcomes = named(found, "toolchain_install_outcome")
    assert len(attempts) == 1, found
    assert len(outcomes) == 1, found
    assert outcomes[0]["outcome"] == "failure"
    assert outcomes[0]["failure_category"] == "rustup_not_found"


def test_bsd_date_falls_back_to_whole_seconds(installer: Installer) -> None:
    """Scenario: ``date`` has no GNU sub-second conversion.

    Invariant: the installer succeeds and every ``elapsed_ms`` field is a
    plain integer. BSD/macOS ``date`` prints ``%3N`` literally, so the stamp
    must be recognised as unusable before it reaches ``elapsed_ms``.
    """
    write_stub(
        installer.root / "stub" / "date",
        """case ${1:-} in
  +%s%3N) printf '17620000003N\\n' ;;
  +%s) printf '1762000000\\n' ;;
  *) printf '1762000000\\n' ;;
esac
""",
    )

    completed = installer.run()

    assert completed.returncode == 0, completed.stdout + completed.stderr
    found = events(completed.stderr)
    assert named(found, "toolchain_install_outcome"), found
    timed = [event for event in found if "elapsed_ms" in event]
    assert timed, found
    for event in timed:
        assert event["elapsed_ms"].isdigit(), (
            f"elapsed_ms should be a plain integer, found {event['elapsed_ms']!r}"
        )