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

The documentation-input classification logic lives in the `repovec-ci` crate.
Keep that helper small and policy-focused.

- Unit coverage uses `rstest`.
- Behavioural coverage uses `rstest-bdd` with
  `strict-compile-time-validation`.
- Workflow shell should stay thin and delegate any classification logic to the
  helper rather than embedding untestable conditions directly in YAML.
- The helper emits conservative-fallback fields, so workflow logs can
  distinguish an actual Mermaid match from an unreadable file that forced
  `make nixie` to run.

### Public API surface

The public API is intentionally small and exposes only the types and functions
the workflow and tests need:

```rust
pub enum DocsGateReason { DocumentationChanged, MissingChangedFiles, NoDocumentationChanges }
impl DocsGateReason { pub const fn as_str(self) -> &'static str }
```

`DocsGateReason` records why the docs gate ran or was skipped and provides the
stable reason string consumed by workflow logging.

```rust
pub struct DocsGatePlan { /* opaque */ }
impl DocsGatePlan {
    pub const fn should_run(&self) -> bool;
    pub const fn docs_gate_required(&self) -> bool;
    pub const fn nixie_required(&self) -> bool;
    pub const fn reason(&self) -> DocsGateReason;
    pub fn matched_files(&self) -> &[String];
    pub fn conservative_fallback_files(&self) -> &[String];
}
```

`DocsGatePlan` is the evaluated policy result, exposing whether the docs gate
should run, whether Mermaid validation is required, which inputs matched, and
which files forced the conservative fallback.

```rust
pub enum MermaidDetection { Present, Absent, Unknown }
```

`MermaidDetection` is the detector result returned for each documentation input
when Mermaid validation is being considered.

```rust
pub fn evaluate_docs_gate_in<I, S>(root: &cap_std::fs_utf8::Dir, changed_files: I) -> DocsGatePlan
where I: IntoIterator<Item = S>, S: AsRef<str>;
```

`evaluate_docs_gate_in` evaluates the policy against a capability-scoped file
system handle and a changed-file list.

```rust
pub fn evaluate_docs_gate_with<I, S, F>(changed_files: I, path_contains_mermaid: F) -> DocsGatePlan
where I: IntoIterator<Item = S>, S: AsRef<str>, F: FnMut(&str) -> MermaidDetection;
```

`evaluate_docs_gate_with` evaluates the same policy with an injected Mermaid
detector, which keeps the policy logic testable without needing a real file
system.

### `cap-std` rationale

`repovec-ci` uses `cap-std` with the `fs_utf8` feature instead of `std::fs`
because it provides capability-safe, UTF-8-native file-system access. That
makes the file-reading boundary explicit in `evaluate_docs_gate_in`, while
`evaluate_docs_gate_with` can be tested entirely through an injected detector
and does not require any real file system.

### `repovec-ci` binary

The `repovec-ci` CLI is invoked by the `docs-gate` job, and its `stdout`
output is appended directly to `$GITHUB_OUTPUT`.

```text
USAGE:
    repovec-ci [--changed-file <path>]... [--help]
    repovec-ci --stdin
```

| Option | Meaning |
| --- | --- |
| `--changed-file <path>` | Treat `<path>` as a changed file. This flag is repeatable and is mutually exclusive with `--stdin`. |
| `--stdin` | Read newline-delimited changed-file paths from standard input. This flag is mutually exclusive with `--changed-file`. |
| `-h`, `--help` | Print usage text and exit. |

The binary writes `key=value` lines to `stdout` for workflow consumption:

- `should_run`
- `docs_gate_required`
- `nixie_required`
- `reason`
- `matched_count`
- `matched_files`
- `conservative_fallback_count`
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
