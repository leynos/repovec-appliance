#!/usr/bin/env bash
# Fast capability check for the repository's development build standard.
#
# Runs before every build target so a missing tool produces an actionable
# installation hint rather than an opaque linker failure deep inside a Cargo
# invocation. Exits non-zero when a required component is absent, unusable, or
# does not match its pin.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/build-tools-common.sh
. "$script_dir/build-tools-common.sh"

# Report on the linker half of the prerequisites. Returns non-zero when mold is
# required but missing, unusable, or a different version from the pin. Only a
# non-Linux host is tolerated, with a note explaining what is used instead.
check_mold() {
  local pinned=$1 installed resolved
  if ! is_linux; then
    note "mold is Linux-only; falling back to the default $(uname -s) linker"
    return 0
  fi
  if ! resolved=$(command -v mold 2>/dev/null); then
    note "mold not found on PATH (pinned $pinned)"
    note 'install it with: make install-build-tools'
    return 1
  fi
  # A mold that cannot report its version is broken -- a truncated download or
  # an unresolved shared library -- so treat it as a failure rather than
  # letting the empty string surface as a confusing version-drift warning.
  if ! installed=$(installed_mold_version) || [ -z "$installed" ]; then
    note "mold at $resolved is on PATH but cannot report its version"
    note 'reinstall it with: make install-build-tools'
    return 1
  fi
  # Report the resolved path, not just the version: `-fuse-ld=mold` selects by
  # PATH order, so naming the winner makes an unexpected pick obvious.
  #
  # A drift from the pin fails rather than warns. An advisory pin is not a pin:
  # tolerating it means the linker actually used, and so any linker-specific
  # behaviour, silently stops matching what the repository claims.
  # `make install-build-tools` puts the pinned release ahead of a distribution
  # one on PATH, so the remedy is a single command.
  if [ "$installed" != "$pinned" ]; then
    note "mold $installed at $resolved does not match the pin $pinned"
    note 'run make install-build-tools to match'
    return 1
  fi
  note "mold $installed at $resolved"
}

# Report on the toolchain half of the prerequisites: rustup itself and the
# pinned nightly. Any absence is fatal, because the standard's parallel
# frontend is a nightly-only flag.
check_toolchain() {
  local toolchain=$1 host installed resolved
  if ! command -v rustup >/dev/null 2>&1; then
    note 'rustup not found on PATH; it is required to select the pinned nightly'
    note 'install it from https://rustup.rs'
    return 1
  fi
  # The installed identity is `<channel>-<host triple>`, and only rustup knows
  # the triple it resolved for this machine. A guessed suffix would disagree
  # with rustup on a cross-compiled or renamed target.
  if ! resolved=$(rustup show); then
    note 'rustup show failed; cannot resolve the host triple'
    return 1
  fi
  host=$(printf '%s\n' "$resolved" | awk -F': ' '/^Default host: / { print $2 }')
  if [ -z "$host" ]; then
    note 'rustup show did not report a default host triple'
    return 1
  fi
  if ! installed=$(rustup toolchain list); then
    note 'rustup toolchain list failed; cannot confirm the pinned toolchain'
    return 1
  fi
  # Compare against the complete identity rather than a prefix: a custom
  # toolchain named after the pin, such as `nightly-2026-03-12-custom`, would
  # otherwise satisfy the check while the pinned host build stayed absent.
  # Read the whole list before deciding so `pipefail` never sees a broken pipe
  # from an early-exiting consumer.
  if ! printf '%s\n' "$installed" |
    awk -v expected="$toolchain-$host" '$1 == expected { found = 1 } END { exit !found }'
  then
    note "toolchain $toolchain for $host is not installed"
    note 'install it with: make install-build-tools'
    return 1
  fi
  note "toolchain $toolchain for $host available"
}

# Run both checks unconditionally so a developer sees every missing piece in
# one pass rather than fixing them one failed run at a time.
main() {
  local status=0 mold_pin toolchain_pin
  # Resolve the pins into variables first. `fail` exits, but inside a command
  # substitution that exit kills only the subshell, so passing `$(mold_version)`
  # straight into a check would continue with an empty pin and report a
  # nonsensical drift. An assignment propagates the status, so this stops.
  mold_pin=$(mold_version) || return 1
  toolchain_pin=$(pinned_toolchain) || return 1
  check_mold "$mold_pin" || status=1
  check_toolchain "$toolchain_pin" || status=1
  [ "$status" -eq 0 ] || note 'capability check failed; see the messages above'
  return "$status"
}

main "$@"
