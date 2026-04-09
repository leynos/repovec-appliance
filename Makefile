.PHONY: help all clean test build release lint whitaker-lint typecheck fmt check-fmt markdownlint nixie


CARGO ?= cargo
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
TEST_CMD := $(if $(shell $(CARGO) nextest --version 2>/dev/null),nextest run --no-tests pass,test)
HAS_DOCTEST_TARGETS := $(shell $(CARGO) metadata --no-deps --format-version 1 2>/dev/null | grep -q '"doctest":true' && echo 1)
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie

build: ## Build the entire workspace in debug mode
	$(CARGO) build --workspace $(BUILD_JOBS)

release: ## Build the entire workspace in release mode
	$(CARGO) build --workspace --release $(BUILD_JOBS)

all: check-fmt lint test ## Perform a comprehensive check of code

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(CARGO) $(TEST_CMD) $(TEST_FLAGS) $(BUILD_JOBS)
ifneq ($(TEST_CMD),test)
ifneq ($(HAS_DOCTEST_TARGETS),)
	RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(CARGO) test --doc $(DOCTEST_FLAGS) $(BUILD_JOBS)
endif
endif

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(EFFECTIVE_RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	$(CARGO) clippy --workspace $(CLIPPY_FLAGS)
	$(MAKE) whitaker-lint

whitaker-lint: ## Run Whitaker when available
	@if command -v whitaker >/dev/null 2>&1; then \
		RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" whitaker --all -- $(CARGO_FLAGS); \
	else \
		echo "whitaker not found on PATH; skipping whitaker lint. Install whitaker to run this check."; \
	fi

typecheck: ## Type-check without building
	RUSTFLAGS="$(EFFECTIVE_RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	$(MDLINT) '**/*.md'

nixie: ## Validate Mermaid diagrams
	$(NIXIE) --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
