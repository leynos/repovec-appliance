.PHONY: help all clean test build release lint whitaker-lint typecheck fmt check-fmt markdownlint docs docs-lint docs-check ensure-cargo nixie spelling build-docs-gate-helper validate-systemd integration-test integration-command-test test-workflow-contracts script-test install-build-tools check-build-tools _check-python _check-integration-prereqs _check-command-test-prereqs


CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
WHITAKER ?= whitaker
PYTHON ?= python3
INTEGRATION_TESTS_DIR ?= integration-tests
PYTEST_FLAGS ?=
BUILD_JOBS ?=
# The development build standard: the pinned `mold` linker and the parallel
# `rustc` frontend are the defaults for development, test, lint, docs and
# typecheck builds. `.cargo/config.toml` carries them so a bare `cargo`
# invocation gets them too; release builds are excluded there and below. The
# toolchain is not pinned separately: the standard uses the repository's own
# nightly from `rust-toolchain.toml`.
MOLD_VERSION_FILE ?= tools/mold/VERSION
MOLD_SHA256SUMS_FILE ?= tools/mold/SHA256SUMS
BUILD_TOOLS_PREFIX ?= $(HOME)/.local
# Exported rather than interpolated into the recipes. Make hands an exported
# variable to the child process directly, so a path containing a quote cannot
# break the command line the shell parses; a `VAR='$(VAR)'` prefix could.
export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE
export BUILD_TOOLS_PREFIX

# Cargo picks a single `rustflags` source rather than merging them, and an
# externally assigned `RUSTFLAGS` outranks every `rustflags` table in
# `.cargo/config.toml`. CI's `setup-rust` exports `-D warnings` for a whole
# job, so without restating the standard here the gates would silently fall
# back to the platform linker and a single-threaded frontend while still
# reporting success.
STANDARD_THREADS_FLAG ?= -Zthreads=8
STANDARD_MOLD_FLAG ?= -Clink-arg=-fuse-ld=mold
# `mold` ships for Linux only. macOS and Windows keep their platform linker,
# matching the `cfg(target_os = "linux")` gate in `.cargo/config.toml`.
BUILD_HOST_OS := $(shell uname -s)
# A failed `uname -s` yields an empty value, which would silently select the
# non-Linux flags. Stop at parse time instead: every route then fails with the
# same diagnosis rather than building with the wrong linker.
ifeq ($(strip $(BUILD_HOST_OS)),)
$(error cannot determine the host operating system: uname -s failed)
endif
STANDARD_RUSTFLAGS = $(STANDARD_THREADS_FLAG)$(if $(filter Linux,$(BUILD_HOST_OS)), $(STANDARD_MOLD_FLAG))
# Make reads an exported `RUSTFLAGS` into this variable. Appending to it is
# what keeps a caller's assignment -- CI's `-D warnings` in particular -- from
# displacing the standard. `$(value ...)` returns the caller's text unexpanded:
# expanding the variable first would consume a literal `$` as a Make reference
# before the value could be quoted for the shell.
INHERITED_RUSTFLAGS := $(strip $(value RUSTFLAGS))

BASE_RUST_FLAGS ?= -D warnings
BASE_RUSTDOC_FLAGS ?= -D warnings
RUST_FLAGS ?=
RUSTDOC_FLAGS ?=
# The warning policy before the standard is composed in. Clippy receives this
# after `--`; the standard reaches every crate through `RUSTFLAGS` instead, so
# a linker flag is not repeated as an explicit compiler argument.
WARNING_RUST_FLAGS := $(strip $(BASE_RUST_FLAGS) $(RUST_FLAGS))
EFFECTIVE_RUST_FLAGS := $(strip $(WARNING_RUST_FLAGS) $(STANDARD_RUSTFLAGS))
EFFECTIVE_RUSTDOC_FLAGS := $(strip $(BASE_RUSTDOC_FLAGS) $(RUSTDOC_FLAGS))
CARGO_FLAGS ?= --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(WARNING_RUST_FLAGS)
TEST_FLAGS ?= $(CARGO_FLAGS)
DOCTEST_FLAGS ?= --workspace --all-features
# Quote a value as one shell word. Single quotes keep every metacharacter the
# caller might have passed -- `"`, a backtick, `$` -- literal, but a single
# quote cannot appear inside them. The standard `'\''` idiom closes the word,
# escapes the quote, and reopens it; `subst` performs that rewrite at Make
# level so the shell only ever sees finished, escaped text.
shell_quote = '$(subst ','\'',$1)'
# Gate routes deny warnings and take the standard, appended to whatever
# `RUSTFLAGS` the caller set. The value is expanded here rather than in the
# shell so `make --dry-run` prints the flags actually used, which the workflow
# contracts evaluate.
GATE_RUSTFLAG_VALUE := $(strip $(INHERITED_RUSTFLAGS) $(EFFECTIVE_RUST_FLAGS))
GATE_RUSTFLAGS = RUSTFLAGS=$(call shell_quote,$(GATE_RUSTFLAG_VALUE))
# A debug build keeps the caller's warning policy rather than imposing one,
# but still takes the standard.
DEBUG_RUSTFLAG_VALUE := $(strip $(INHERITED_RUSTFLAGS) $(STANDARD_RUSTFLAGS))
DEBUG_RUSTFLAGS = RUSTFLAGS=$(call shell_quote,$(DEBUG_RUSTFLAG_VALUE))
# Release builds take neither the parallel frontend nor `mold`. Assigning
# `RUSTFLAGS` at all -- even to an empty inherited value -- displaces the
# configuration file's `rustflags` tables, which is the whole mechanism. This
# route must not depend on the build tools being installed.
RELEASE_RUSTFLAGS = RUSTFLAGS=$(call shell_quote,$(INHERITED_RUSTFLAGS))
# `-fuse-ld=mold` and `command -v mold` both resolve by PATH order, so the
# pinned install prefix leads. An overridden BUILD_TOOLS_PREFIX is therefore
# the prefix the linker actually comes from, not a hard-coded ~/.local.
export PATH := $(BUILD_TOOLS_PREFIX)/bin:$(PATH)
MDLINT ?= $(shell command -v markdownlint-cli2 2>/dev/null || printf '%s' "$$HOME/.bun/bin/markdownlint-cli2")
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# Both modes need mdtablefix 0.6.0 or later; CI pins the version at the
# install-mdtablefix step.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
NIXIE ?= nixie
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
RUFF_VERSION ?= 0.15.12
PYTEST_VERSION ?= 9.0.2
PYTEST_COV_VERSION ?= 7.0.0
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.3
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --python 3.14 --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
CMD_MOX_VERSION ?= 0.2.0
CYCLOPTS_VERSION ?= 4.25.0
PLUMBUM_VERSION ?= 2.0.2
SCRIPT_PY_SRCS := scripts/run_rust_tests.py scripts/tests/test_run_rust_tests.py
SCRIPT_COVERAGE_ARGS := --cov=run_rust_tests --cov-fail-under=90
# The runner's own dependencies travel in its uv script block; they are
# repeated here because the test process imports it as a module rather
# than launching it as a script.
SCRIPT_PYTEST = PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project \
	--python 3.13 --with cmd-mox==$(CMD_MOX_VERSION) \
	--with cyclopts==$(CYCLOPTS_VERSION) --with plumbum==$(PLUMBUM_VERSION) \
	--with pytest==$(PYTEST_VERSION) --with pytest-cov==$(PYTEST_COV_VERSION) \
	python -m pytest
# One command, so no gate status can be discarded by a later one. The
# sequencing and its failure handling live in the script, under test.
RUST_TEST_RUNNER = $(UV_ENV) $(UV) run --python 3.13 scripts/run_rust_tests.py

ensure-cargo: ## Validate cargo toolchain is available for Rust targets
	@if { command -v "$(CARGO)" >/dev/null 2>&1 || test -x "$(CARGO)"; } then \
		:; \
	else \
		echo "cargo executable not found; set CARGO or install cargo at $(HOME)/.cargo/bin/cargo"; \
		exit 1; \
	fi

install-build-tools: ## Install the pinned mold linker and the pinned toolchain
	@scripts/install-build-tools.sh

check-build-tools: ## Check the mold linker and toolchain prerequisites
	@scripts/check-build-tools.sh

build: ensure-cargo check-build-tools ## Build the entire workspace in debug mode
	$(DEBUG_RUSTFLAGS) $(CARGO) build --workspace $(BUILD_JOBS)

release: ensure-cargo ## Build the entire workspace in release mode
	$(RELEASE_RUSTFLAGS) $(CARGO) build --workspace --release $(BUILD_JOBS)

all: check-fmt lint test spelling ## Perform a comprehensive check of code and prose

clean: ensure-cargo ## Remove build artefacts
	$(CARGO) clean

test: ensure-cargo check-build-tools script-test ## Run tests with warnings treated as errors
	$(RUST_TEST_RUNNER) --cargo="$(CARGO)" --rust-flags=$(call shell_quote,$(GATE_RUSTFLAG_VALUE)) --test-flags="$(TEST_FLAGS)" --doctest-flags="$(DOCTEST_FLAGS)" --build-jobs="$(BUILD_JOBS)"

lint: ensure-cargo check-build-tools ## Run Clippy and the Whitaker Dylint suite with warnings denied
	$(GATE_RUSTFLAGS) RUSTDOCFLAGS="$(EFFECTIVE_RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	$(GATE_RUSTFLAGS) $(CARGO) clippy --workspace $(CLIPPY_FLAGS)
	$(MAKE) whitaker-lint

whitaker-lint: ## Run the Whitaker Dylint suite when available
	@if command -v "$(WHITAKER)" >/dev/null 2>&1; then \
		$(GATE_RUSTFLAGS) $(WHITAKER) --all -- $(CARGO_FLAGS); \
	else \
		echo "whitaker not found on PATH; skipping whitaker lint. Install whitaker to run this check."; \
	fi

typecheck: ensure-cargo check-build-tools ## Type-check without building
	$(GATE_RUSTFLAGS) $(CARGO) check $(CARGO_FLAGS)

fmt: ensure-cargo ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	@unset FORCE_COLOR; $(MDLINT) --fix "**/*.md"

check-fmt: ensure-cargo ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

markdownlint: spelling ## Lint Markdown files and enforce spelling
	@if command -v "$(MDLINT)" >/dev/null 2>&1 || test -x "$(MDLINT)"; then \
		$(MDLINT) '**/*.md'; \
	else \
		echo "markdownlint-cli2 executable not found; set MDLINT or install markdownlint-cli2 at $(HOME)/.bun/bin/markdownlint-cli2"; \
		exit 1; \
	fi

spelling: ## Enforce en-GB-oxendict spelling
	$(TYPOS_CONFIG_BUILDER) gate --repository .

docs-lint: ## Backwards-compatible docs lint target
	$(MAKE) markdownlint

docs-check: ## Alias for docs lint checks
	$(MAKE) markdownlint

docs: ## Legacy docs target alias
	$(MAKE) markdownlint

nixie: ## Validate Mermaid diagrams
	$(NIXIE) --no-sandbox

build-docs-gate-helper: ensure-cargo check-build-tools ## Build the repovec-ci docs-gate helper
	$(DEBUG_RUSTFLAGS) $(CARGO) build --quiet -p repovec-ci

validate-systemd: ensure-cargo check-build-tools ## Validate checked-in systemd unit contracts
	$(DEBUG_RUSTFLAGS) $(CARGO) build --quiet -p repovec-ci
	$(DEBUG_RUSTFLAGS) $(CARGO) run --quiet -p repovec-ci -- systemd-gate

_check-python: ## (internal) Print a skip message if $(PYTHON) is missing
	@command -v "$(PYTHON)" >/dev/null 2>&1 \
		|| { echo "$(PYTHON) not found on PATH; skipping integration test target."; exit 1; }

_check-integration-prereqs: _check-python ## (internal) Check lifecycle prereqs
	@"$(PYTHON)" -c 'import pytest, testcontainers' >/dev/null 2>&1 \
		|| { echo "pytest or testcontainers not installed; skipping integration-test."; echo "Install dependencies via: cd $(INTEGRATION_TESTS_DIR) && uv sync"; exit 1; }
	@"$(PYTHON)" -c 'import docker; docker.from_env().ping()' >/dev/null 2>&1 \
		|| { echo "No Docker-compatible runtime reachable; skipping integration-test."; echo "Start Podman: podman system service --time=0 & export DOCKER_HOST=unix://\$$XDG_RUNTIME_DIR/podman/podman.sock"; exit 1; }

_check-command-test-prereqs: _check-python ## (internal) Check cmd-mox prereqs
	@"$(PYTHON)" -c 'import pytest, cmd_mox, cuprum' >/dev/null 2>&1 \
		|| { echo "pytest, cmd-mox or cuprum not installed; skipping integration-command-test."; echo "Install dependencies via: cd $(INTEGRATION_TESTS_DIR) && uv sync"; exit 1; }

integration-test: _check-integration-prereqs ## Run testcontainers-based provisioning lifecycle tests
	cd $(INTEGRATION_TESTS_DIR) && "$(PYTHON)" -m pytest -m integration provisioning $(PYTEST_FLAGS)

integration-command-test: _check-command-test-prereqs ## Run cmd-mox-based command-contract tests
	cd $(INTEGRATION_TESTS_DIR) && "$(PYTHON)" -m pytest -m cmd_mox provisioning $(PYTEST_FLAGS)

script-test: ## Validate the Rust test-gate runner script
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SCRIPT_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SCRIPT_PY_SRCS)
	@$(SCRIPT_PYTEST) scripts/tests/test_run_rust_tests.py -c /dev/null --rootdir=. -p no:cacheprovider -p cmd_mox.pytest_plugin --no-cmd-mox-auto-lifecycle $(SCRIPT_COVERAGE_ARGS)

test-workflow-contracts: ## Validate the mutation-testing caller contract
	uv run --with 'pytest>=8' --with 'pyyaml>=6' --with 'hypothesis>=6' pytest tests/workflow_contracts -q

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
