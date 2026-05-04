# Developers guide

This guide is for maintainers and contributors working on repovec-appliance. It
describes the repository-level build, test, lint, and continuous integration
(CI) workflow that must remain true as the project evolves.

## Normative references

- [Documentation contents](contents.md) if present
- [repovec-appliance technical design](repovec-appliance-technical-design.md)
- [Roadmap](roadmap.md)
- [Execution plan for 1.1.3: CI gating pipeline](execplans/1-1-3-ci-gating-pipeline.md)

## 1. Local quality gates

Before proposing a code change, run the repository gate targets:

```sh
set -o pipefail
make build 2>&1 | tee /tmp/repovec-make-build.log
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-make-check-fmt.log
set -o pipefail
make lint 2>&1 | tee /tmp/repovec-make-lint.log
set -o pipefail
make test 2>&1 | tee /tmp/repovec-make-test.log
```

When a change touches Markdown or documentation-tooling configuration, also
run:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/repovec-make-fmt.log
set -o pipefail
make markdownlint 2>&1 | tee /tmp/repovec-make-markdownlint.log
set -o pipefail
make nixie 2>&1 | tee /tmp/repovec-make-nixie.log
```

These Make targets are the source of truth for local validation and for CI. Do
not duplicate or partially reimplement them in workflow YAML.

## 2. GitHub Actions gate set

The repository CI workflow exposes five stable, required job names:

- `build`
- `check-fmt`
- `lint`
- `test`
- `docs-gate`

The first four jobs run on pull request updates, pushes to `main`, and manual
workflow dispatch.

`docs-gate` always reports a result so it can be configured as a required
check. It runs `make markdownlint` and `make nixie` only when the changed-file
set includes a documentation input. Markdown inputs use one of these extensions:

- `.md`
- `.markdown`
- `.mdx`

Documentation-tooling configuration changes also count as documentation input:

- `.markdownlint-cli2.jsonc`

When the changed-file list is unavailable, the workflow runs the documentation
gate conservatively instead of risking a skipped validation. That fallback
requires both `make markdownlint` and `make nixie`.

`make nixie` is narrower than `make markdownlint`: Mermaid validation runs only
when one of the changed Markdown files contains a Mermaid diagram, or when a
documentation-tooling configuration change or missing changed-file input
requires the conservative path. The user-visible flow is documented in
[users-guide.md](users-guide.md).

## 3. CI policy helper

### 3.1 Public API

```rust
/// Reason the documentation gate should or should not run.
pub enum DocsGateReason { DocumentationChanged, MissingChangedFiles, NoDocumentationChanges }
impl DocsGateReason { pub const fn as_str(self) -> &'static str }

/// Computed plan for the documentation gate.
pub struct DocsGatePlan { /* opaque */ }
impl DocsGatePlan {
    pub const fn should_run(&self) -> bool;
    pub const fn docs_gate_required(&self) -> bool;
    pub const fn nixie_required(&self) -> bool;
    pub const fn reason(&self) -> DocsGateReason;
    pub fn matched_files(&self) -> &[String];
    pub fn conservative_fallback_files(&self) -> &[String];
}

/// Whether a Markdown file contains a Mermaid diagram (or could not be read).
pub enum MermaidDetection { Present, Absent, Unknown }

/// Evaluates the docs-gate policy using the real file system.
pub fn evaluate_docs_gate_in<I, S>(root: &cap_std::fs_utf8::Dir, changed_files: I) -> DocsGatePlan
where I: IntoIterator<Item = S>, S: AsRef<str>;

/// Evaluates the docs-gate policy with an injected Mermaid detector.
/// Use this in tests to avoid real file I/O.
pub fn evaluate_docs_gate_with<I, S, F>(changed_files: I, path_contains_mermaid: F) -> DocsGatePlan
where I: IntoIterator<Item = S>, S: AsRef<str>, F: FnMut(&str) -> MermaidDetection;
```

`DocsGateReason` records why the documentation gate should run or be skipped.
`DocsGatePlan` is the computed result that the workflow consumes. `MermaidDetection`
captures whether a Markdown file contains Mermaid content or falls back to the
conservative unreadable-file path. `evaluate_docs_gate_in` evaluates the policy
with the real file system, and `evaluate_docs_gate_with` evaluates the same
policy with an injected detector.

`evaluate_docs_gate_in` uses `cap_std::fs_utf8::Dir` to read file contents.
`evaluate_docs_gate_with` accepts an injected closure, making the policy fully
testable without any file I/O.

### 3.2 `cap-std` dependency rationale

`cap-std` (with the `fs_utf8` feature) is used instead of `std::fs` because it
provides capability-safe, UTF-8-native file-system access; this makes the
file-reading boundary explicit and keeps the core policy logic
(`evaluate_docs_gate_with`) free of ambient authority, making it straightforward
to test with a stub closure.

### 3.3 `repovec-ci` binary

```text
USAGE:
    repovec-ci [--changed-file <path> [--changed-file <path>]...] [--help]
    repovec-ci --stdin
```

| Flag | Description |
| --- | --- |
| `--changed-file <path>` | Treat `<path>` as a changed file (repeatable; mutually exclusive with `--stdin`) |
| `--stdin` | Read newline-delimited changed-file paths from stdin (mutually exclusive with `--changed-file`) |
| `-h`, `--help` | Print usage text and exit |

The binary writes `key=value` lines to stdout for use with `$GITHUB_OUTPUT`
and is invoked by the `docs-gate` CI job. The output keys are:

- `should_run`
- `docs_gate_required`
- `nixie_required`
- `reason`
- `matched_files_count`
- `matched_files`
- `conservative_fallback_files_count`
- `conservative_fallback_files`

## 4. Required-check enforcement

The desired repository ruleset is versioned in
[`/.github/rulesets/main-ci-gating.json`](../.github/rulesets/main-ci-gating.json).
 Apply that payload only after the workflow changes that produce the required
checks are available on the default branch.

Example application command:

```sh
gh api \
  --method POST \
  repos/leynos/repovec-appliance/rulesets \
  --input .github/rulesets/main-ci-gating.json
```

If a ruleset named `main-ci-gating` already exists, update it instead:

```sh
RULESET_ID="$(gh api repos/leynos/repovec-appliance/rulesets --jq '.[] | select(.name == "main-ci-gating") | .id')"
gh api \
  --method PUT \
  "repos/leynos/repovec-appliance/rulesets/${RULESET_ID}" \
  --input .github/rulesets/main-ci-gating.json
```

Keep the workflow job names and the ruleset payload synchronized. Changing a
required job name without updating the ruleset will block merges.

## 5. Appliance module

### 5.1 Appliance module overview

`crates/repovec-core/src/appliance/` contains appliance-specific validation
helpers. Keep these helpers close to the assets they validate and avoid placing
appliance policy in unrelated core modules.

Each appliance asset gets its own submodule under `appliance/`. The submodule
owns the checked-in asset contract, any local parsing needed for that asset,
the typed validation error, and focused tests for the contract.

### 5.2 `qdrant_quadlet` validation surface

The `qdrant_quadlet` module exposes the public validation surface for the
checked-in Qdrant Podman Quadlet:

- `checked_in_qdrant_quadlet() -> &'static str` returns the repository's
  embedded Quadlet source.
- `validate_checked_in_qdrant_quadlet() -> Result<(), QdrantQuadletError>`
  validates the embedded Quadlet asset.
- `validate_qdrant_quadlet(contents: &str) -> Result<(), QdrantQuadletError>`
  validates caller-provided Quadlet contents against the same appliance
  contract.
- `QdrantQuadletError` is the typed error enum for validation failures. See
  `crates/repovec-core/src/appliance/qdrant_quadlet/error.rs` for the full
  variant list.

### 5.3 Extension pattern

To add validation for a new appliance asset:

1. Create a submodule directory under `appliance/` with `mod.rs`, `error.rs`,
   `parser.rs` if a custom parser is needed, and `tests.rs`.
2. Re-export the submodule from `appliance/mod.rs`.
3. Embed the checked-in asset with `include_str!` and expose a
   `checked_in_*()` function.
4. Write `validate_*()` returning a typed error enum that implements
   `std::error::Error` and `fmt::Display`.
5. Cover all error variants in `tests.rs` using `rstest` fixtures and add BDD
   scenarios under `crates/repovec-core/tests/features/`.

### 5.4 Test patterns

The appliance validation modules use `rstest` for unit tests and
`rstest-bdd` with `rstest-bdd-macros` for behavioural tests. Unit tests should
exercise each typed error variant directly. Behavioural tests should describe
the appliance contract in feature files and keep the executable scenarios thin.

See [rstest BDD users guide](rstest-bdd-users-guide.md) and
[Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md) for
the project-local testing guidance.
