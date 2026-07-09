.PHONY: help all clean test build release lint whitaker-lint typecheck fmt check-fmt markdownlint docs docs-lint docs-check ensure-cargo nixie validate-systemd integration-test integration-command-test test-workflow-contracts _check-python _check-integration-prereqs _check-command-test-prereqs


CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
WHITAKER ?= whitaker
PYTHON ?= python3
INTEGRATION_TESTS_DIR ?= integration-tests
PYTEST_FLAGS ?=
BUILD_JOBS ?=
BASE_RUST_FLAGS ?= -D warnings
BASE_RUSTDOC_FLAGS ?= -D warnings
RUST_FLAGS ?=
RUSTDOC_FLAGS ?=
EFFECTIVE_RUST_FLAGS := $(strip $(BASE_RUST_FLAGS) $(RUST_FLAGS))
EFFECTIVE_RUSTDOC_FLAGS := $(strip $(BASE_RUSTDOC_FLAGS) $(RUSTDOC_FLAGS))
CARGO_FLAGS ?= --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(EFFECTIVE_RUST_FLAGS)
TEST_FLAGS ?= $(CARGO_FLAGS)
DOCTEST_FLAGS ?= --workspace --all-features
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
NIXIE ?= nixie

ensure-cargo: ## Validate cargo toolchain is available for Rust targets
	@if { command -v "$(CARGO)" >/dev/null 2>&1 || test -x "$(CARGO)"; } then \
		:; \
	else \
		echo "cargo executable not found; set CARGO or install cargo at $(HOME)/.cargo/bin/cargo"; \
		exit 1; \
	fi

build: ensure-cargo ## Build the entire workspace in debug mode
	$(CARGO) build --workspace $(BUILD_JOBS)

release: ensure-cargo ## Build the entire workspace in release mode
	$(CARGO) build --workspace --release $(BUILD_JOBS)

all: check-fmt lint test ## Perform a comprehensive check of code

clean: ensure-cargo ## Remove build artifacts
	$(CARGO) clean

test: ensure-cargo ## Run tests with warnings treated as errors
	@if $(CARGO) nextest --version >/dev/null 2>&1; then \
		TEST_CMD="nextest run --no-tests pass"; \
	else \
		TEST_CMD="test"; \
	fi; \
	RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(CARGO) $$TEST_CMD $(TEST_FLAGS) $(BUILD_JOBS); \
	if [ "$$TEST_CMD" != "test" ] && $(CARGO) metadata --no-deps --format-version 1 2>/dev/null | grep -q '"doctest":true'; then \
		RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(CARGO) test --doc $(DOCTEST_FLAGS) $(BUILD_JOBS); \
	fi

lint: ensure-cargo ## Run Clippy and the Whitaker Dylint suite with warnings denied
	RUSTDOCFLAGS="$(EFFECTIVE_RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	$(CARGO) clippy --workspace $(CLIPPY_FLAGS)
	$(MAKE) whitaker-lint

whitaker-lint: ## Run the Whitaker Dylint suite when available
	@if command -v "$(WHITAKER)" >/dev/null 2>&1; then \
		RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS); \
	else \
		echo "whitaker not found on PATH; skipping whitaker lint. Install whitaker to run this check."; \
	fi

typecheck: ensure-cargo ## Type-check without building
	RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS)

fmt: ensure-cargo ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ensure-cargo ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	@if command -v "$(MDLINT)" >/dev/null 2>&1 || test -x "$(MDLINT)"; then \
		$(MDLINT) '**/*.md'; \
	else \
		echo "markdownlint-cli2 executable not found; set MDLINT or install markdownlint-cli2 at $(HOME)/.bun/bin/markdownlint-cli2"; \
		exit 1; \
	fi

docs-lint: ## Backwards-compatible docs lint target
	$(MAKE) markdownlint

docs-check: ## Alias for docs lint checks
	$(MAKE) markdownlint

docs: ## Legacy docs target alias
	$(MAKE) markdownlint

nixie: ## Validate Mermaid diagrams
	$(NIXIE) --no-sandbox

validate-systemd: ensure-cargo ## Validate checked-in systemd unit contracts
	$(CARGO) build --quiet -p repovec-ci
	$(CARGO) run --quiet -p repovec-ci -- systemd-gate

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

test-workflow-contracts: ## Validate the mutation-testing caller contract
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
