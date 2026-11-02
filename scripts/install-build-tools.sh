#!/usr/bin/env bash
# Install the build tools the repository's development standard needs.
#
# Downloads the pinned mold release, verifies it against
# tools/mold/SHA256SUMS, unpacks it under $BUILD_TOOLS_PREFIX (default
# ~/.local), then installs the pinned nightly toolchain. Nothing here builds a
# tool from source, and nothing here touches a release or packaging toolchain.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/build-tools-common.sh
. "$script_dir/build-tools-common.sh"

MOLD_RELEASE_BASE_URL_DEFAULT=https://github.com/rui314/mold/releases/download
MOLD_RELEASE_BASE_URL=${MOLD_RELEASE_BASE_URL:-$MOLD_RELEASE_BASE_URL_DEFAULT}

# Download bounds, overridable so a test can drive them without waiting.
CURL_CONNECT_TIMEOUT=${CURL_CONNECT_TIMEOUT:-15}
CURL_MIN_BYTES_PER_SECOND=${CURL_MIN_BYTES_PER_SECOND:-1024}
CURL_STALL_SECONDS=${CURL_STALL_SECONDS:-60}
# The download is retried a bounded number of times inside a bounded window.
# `CURL_RETRIES` counts additional attempts after the first, so the total is
# `CURL_RETRIES + 1`. The loop is explicit rather than curl's `--retry` so
# every attempt and outcome is observable as an event.
CURL_RETRIES=${CURL_RETRIES:-3}
CURL_RETRY_DELAY_SECONDS=${CURL_RETRY_DELAY_SECONDS:-2}
CURL_RETRY_MAX_SECONDS=${CURL_RETRY_MAX_SECONDS:-60}

# The scratch directory the EXIT trap removes. A trap whose action is a string
# is re-parsed by the shell when it fires, so a path containing a quote -- from
# a quote-bearing `TMPDIR`, say -- breaks the quoting and the cleanup never
# runs. Naming a function instead means the path is only ever a variable,
# expanded at removal time and never re-parsed. It is script-scope rather than
# local because the trap fires after `install_mold` has returned.
BUILD_TOOLS_WORKDIR=

remove_workdir() {
  [ -n "$BUILD_TOOLS_WORKDIR" ] || return 0
  rm -rf -- "$BUILD_TOOLS_WORKDIR"
  BUILD_TOOLS_WORKDIR=
}

trap remove_workdir EXIT

# Milliseconds since the epoch, for bounded elapsed-time events.
#
# GNU `date` supports `+%s%3N`. BSD/macOS `date` has no sub-second
# conversion and prints the `%3N` literally, which would reach the
# arithmetic in `elapsed_ms` as a non-numeric stamp and abort the script
# under `set -e`. Fall back to whole seconds scaled to milliseconds, and
# still propagate a genuine `date` failure.
now_ms() {
  local stamp
  stamp=$(date +%s%3N) || fail 'cannot read the current time: date +%s%3N failed'
  [ -n "$stamp" ] ||
    fail 'cannot read the current time: date +%s%3N returned no output'
  case $stamp in
    *[!0-9]*)
      stamp=$(date +%s) || fail 'cannot read the current time: date +%s failed'
      [ -n "$stamp" ] ||
        fail 'cannot read the current time: date +%s returned no output'
      case $stamp in
        *[!0-9]*) fail "cannot read the current time: date returned '$stamp'" ;;
      esac
      printf '%s' "$((stamp * 1000))"
      ;;
    *) printf '%s' "$stamp" ;;
  esac
}

# Emit one stable, bounded event line. Fields are positional key=value pairs;
# callers must never pass a full URL or a credential.
event() {
  local name=$1
  shift
  printf 'build-tools: event=%s' "$name" >&2
  local field
  for field in "$@"; do
    printf ' %s' "$field" >&2
  done
  printf '\n' >&2
}

# Milliseconds elapsed since a `now_ms` reading.
elapsed_ms() {
  local start=$1
  printf '%s' "$(($(now_ms) - start))"
}

# Categorize the download endpoint instead of naming its host. Event fields are
# the script's bounded metric labels: a host derived from a configurable base
# URL has unbounded cardinality, while the note written below still names the
# full URL for a human diagnosing a failed download.
url_source() {
  if [ "$MOLD_RELEASE_BASE_URL" = "$MOLD_RELEASE_BASE_URL_DEFAULT" ]; then
    printf 'default'
  else
    printf 'override'
  fi
}

# Verify the downloaded tarball against the single matching line in
# SHA256SUMS. An unlisted artefact is a hard failure, never a silent skip.
verify_mold_archive() {
  local archive=$1 name=$2 expected recorded
  expected=$(awk -v name="$name" '$2 == name { print $1 }' "$MOLD_SHA256SUMS_FILE")
  [ -n "$expected" ] || fail "no checksum recorded for $name in $MOLD_SHA256SUMS_FILE"
  # Refuse an ambiguous file rather than guessing. Several rows for one
  # artefact make `expected` multi-line, and the check below would then hand
  # `sha256sum` one malformed line per extra digest plus a single well-formed
  # one. Malformed lines are only warned about, so the verdict would silently
  # rest on whichever digest happened to come last -- a file recording a wrong
  # digest alongside the right one would verify.
  recorded=$(printf '%s\n' "$expected" | grep -c .)
  [ "$recorded" -eq 1 ] ||
    fail "$recorded checksums recorded for $name in $MOLD_SHA256SUMS_FILE; refusing to guess"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status ||
    fail "checksum mismatch for $name; refusing to install"
  note "verified $name against $MOLD_SHA256SUMS_FILE"
}

# Download the release with an explicit, bounded retry loop.
#
# curl's own `--retry` flags hide how many attempts ran and when each one
# failed. An explicit loop makes every attempt observable as an event, and
# keeps the overall window bounded by CURL_RETRY_MAX_SECONDS.
download_mold() {
  local version=$1 arch=$2 name=$3 url=$4 destination=$5
  local attempt=1 max_attempts=$((CURL_RETRIES + 1))
  local started deadline attempt_started status
  started=$(now_ms)
  deadline=$((started + CURL_RETRY_MAX_SECONDS * 1000))
  while :; do
    event mold_download_attempt \
      "attempt=$attempt" "version=$version" "arch=$arch" \
      "url_source=$(url_source)"
    attempt_started=$(now_ms)
    if curl --fail --silent --show-error --location \
      --connect-timeout "$CURL_CONNECT_TIMEOUT" \
      --speed-limit "$CURL_MIN_BYTES_PER_SECOND" --speed-time "$CURL_STALL_SECONDS" \
      --output "$destination" "$url"; then
      event mold_download_outcome "outcome=success" "attempt=$attempt" \
        "elapsed_ms=$(elapsed_ms "$attempt_started")"
      return 0
    else
      status=$?
      event mold_download_outcome "outcome=failure" "attempt=$attempt" \
        "elapsed_ms=$(elapsed_ms "$attempt_started")" \
        "failure_category=curl_status_$status"
    fi
    if [ "$attempt" -ge "$max_attempts" ]; then
      return 1
    fi
    # Stop when the next attempt would start outside the overall window.
    if [ "$(now_ms)" -ge "$deadline" ]; then
      return 1
    fi
    sleep "$CURL_RETRY_DELAY_SECONDS"
    attempt=$((attempt + 1))
  done
}

# Download, verify, and unpack the pinned mold release, or explain why the step
# is being skipped on a platform mold does not support.
install_mold() {
  local version=$1 arch name url workdir host status
  if ! is_linux; then
    host=$(host_os)
    note "mold is Linux-only; skipping on $host, the platform linker is used instead"
    return 0
  fi
  arch=$(mold_arch)
  name="mold-$version-$arch-linux.tar.gz"
  url="$MOLD_RELEASE_BASE_URL/v$version/$name"

  BUILD_TOOLS_WORKDIR=$(mktemp -d)
  workdir=$BUILD_TOOLS_WORKDIR

  note "downloading $url"
  # Bound both the handshake and the transfer. Without these a server that
  # accepts the connection and then stalls leaves `curl` waiting indefinitely,
  # so the failure path below is never reached and `make install-build-tools`
  # simply hangs. The transfer bound is a stall detector rather than a
  # deadline: a plain `--max-time` would punish a slow-but-progressing link,
  # whereas `--speed-limit`/`--speed-time` only fire when throughput dies.
  # The retry loop above bounds the number of attempts and the overall window.
  if ! download_mold "$version" "$arch" "$name" "$url" "$workdir/$name"; then
    event mold_install_outcome "outcome=failure" "version=$version" \
      "arch=$arch" "failure_category=download"
    fail "failed to download $name"
  fi
  verify_mold_archive "$workdir/$name" "$name"

  # The tarball root is mold-<version>-<arch>-linux/{bin,lib,libexec}; strip it
  # so the tree merges into the prefix and `bin/mold` lands on PATH.
  mkdir -p "$BUILD_TOOLS_PREFIX"
  if tar --extract --gzip --strip-components=1 --directory "$BUILD_TOOLS_PREFIX" --file "$workdir/$name"; then
    event mold_extract_outcome "outcome=success" "version=$version" "arch=$arch"
  else
    status=$?
    event mold_extract_outcome "outcome=failure" "version=$version" \
      "arch=$arch" "failure_category=tar_status_$status"
    event mold_install_outcome "outcome=failure" "version=$version" \
      "arch=$arch" "failure_category=extract"
    fail "failed to unpack $name into $BUILD_TOOLS_PREFIX"
  fi
  note "installed mold $version into $BUILD_TOOLS_PREFIX"
  event mold_install_outcome "outcome=success" "version=$version" "arch=$arch"
  # The Makefile prepends this prefix to PATH for every target; the hint
  # matters only when the script is invoked directly.
  note "put $BUILD_TOOLS_PREFIX/bin first on PATH when not using the make targets"
}

# Install the pinned nightly. Uses the minimal profile: this is the toolchain
# `rust-toolchain.toml` already selects, installed eagerly so a missing one is
# reported here rather than mid-build.
install_toolchain() {
  local toolchain=$1 started status
  started=$(now_ms)
  event toolchain_install_attempt "toolchain=$toolchain"
  if ! command -v rustup >/dev/null 2>&1; then
    event toolchain_install_outcome "outcome=failure" "toolchain=$toolchain" \
      "elapsed_ms=$(elapsed_ms "$started")" "failure_category=rustup_not_found"
    fail 'rustup not found on PATH; install it from https://rustup.rs'
  fi
  note "installing toolchain $toolchain"
  if rustup toolchain install "$toolchain" --profile minimal; then
    event toolchain_install_outcome "outcome=success" "toolchain=$toolchain" \
      "elapsed_ms=$(elapsed_ms "$started")"
  else
    status=$?
    event toolchain_install_outcome "outcome=failure" "toolchain=$toolchain" \
      "elapsed_ms=$(elapsed_ms "$started")" "failure_category=rustup_status_$status"
    fail "failed to install toolchain $toolchain"
  fi
}

# Install both halves. The linker step runs first so a checksum failure aborts
# before spending time on a toolchain download.
main() {
  local mold_pin toolchain_pin
  # Resolve the pins into variables first: `fail` exits, but inside a command
  # substitution that exit kills only the subshell, so an unreadable pin would
  # otherwise reach the installer as an empty string and be built into a
  # download URL.
  mold_pin=$(mold_version) || return 1
  toolchain_pin=$(pinned_toolchain) || return 1
  install_mold "$mold_pin"
  install_toolchain "$toolchain_pin"
  note 'ready; verify with: make check-build-tools'
}

main "$@"
