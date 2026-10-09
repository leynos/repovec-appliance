# Implement 1.1 project scaffolding and crate structure

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item 1.1 is the first delivery step for the appliance. After this
change, the repository will no longer be a single placeholder binary. It will
be a Rust workspace that contains the four planned executables `repovecd`,
`repovec-mcpd`, `repovec-tui`, and `repovectl`, plus the shared library
`repovec-core`. The workspace will compile, the lint and formatting policy will
be enforced consistently, and continuous integration will run the same commit
gates on every push.

This change does not make the appliance operational yet. It creates the
delivery skeleton that later roadmap steps depend on, especially the Qdrant
health check in 1.2 and the systemd units in 1.3. Observable success for this
step means:

1. `cargo build --workspace` succeeds.
2. `make check-fmt`, `make lint`, and `make test` succeed with no warnings.
3. The CI workflow runs the commit-gate Make targets on `push` and
   `pull_request`.
4. The design document records the workspace and crate-boundary decisions.
5. The roadmap entries for 1.1 are marked done only after all gates pass.

## Repository orientation

The repository currently contains a single package at the workspace root:
`repovec_appliance` in `Cargo.toml`, with one stub entrypoint in `src/main.rs`.
The Makefile assumes one binary target named `repovec_appliance`. The GitHub
Actions workflow already runs formatting, lint, and coverage-related steps, but
it does not trigger on `push`, and it does not encode branch protection.

The user request referenced `docs/podbot-roadmap.md`, but the repository only
contains `docs/roadmap.md`. This plan assumes `docs/roadmap.md` is the intended
source of truth. If that assumption is wrong, stop and reconcile the source
documents before implementation.

`docs/users-guide.md` does not exist today. Because the project documentation
style expects a users guide and the user requested user-facing documentation
updates, this plan creates a minimal initial users guide during implementation.
That guide will explain the scaffolded binaries and any visible command-line
behaviour introduced by this step.

## Constraints

- Preserve the runtime names used by the roadmap and design document:
  `repovecd`, `repovec-mcpd`, `repovec-tui`, `repovectl`, and `repovec-core`.
- Keep `repovec-core` narrow. It may hold shared types, configuration, path
  conventions, and shared contracts, but daemon-specific lifecycle logic stays
  in the owning binary crate.
- Convert the repository root into a virtual workspace root rather than keeping
  a fifth application package. The current `repovec_appliance` placeholder is
  implementation debt and should not survive this step.
- Carry forward the existing lint intent from the current root `Cargo.toml`,
  including pedantic Clippy, panic-prone denials, and missing documentation
  enforcement.
- Add `rustfmt.toml` at the repository root using the exact project
  conventions supplied in the request, including `fn_single_line = true` and
  `unstable_features = true`.
- Keep the pinned nightly toolchain so `cargo fmt` can honour the requested
  unstable rustfmt option.
- Every new crate must begin with a crate-level or module-level documentation
  comment and must compile under the strict lint profile without local
  suppressions unless there is a documented justification.
- Public APIs added to `repovec-core` must include Rustdoc examples that are
  valid doctests, avoid `.unwrap()` and `.expect()`, and use hidden setup lines
  where necessary.
- Unit tests must use `rstest` fixtures and parameterization where useful.
- Behavioural tests must use `rstest-bdd` version `0.5.0` and cover happy
  paths, unhappy paths, and at least one edge case for the new observable
  scaffold behaviour.
- Documentation changes for this step must include:
  `docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`, and
  `docs/roadmap.md`.
- The implementation phase must not begin until the user approves this plan.

## Tolerances

- If converting the root package into a virtual workspace requires deleting the
  existing `src/main.rs`, that is in scope. If other unrelated root-level files
  need removal, stop and document why.
- If a planned Make target cannot work for a virtual workspace without broader
  behavioural changes, update the target in scope for 1.1. If the change would
  break later roadmap assumptions, stop and escalate.
- If CI merge gating requires repository settings or branch-protection changes
  that cannot be expressed from this checkout, update the workflow files and
  document the exact manual settings required. Do not claim merge protection is
  complete unless it is verifiably configured.
- If `rstest-bdd` `0.5.0` or its macro validation mode conflicts with the
  pinned toolchain or the workspace layout, stop after capturing the failing
  evidence and propose the smallest viable adjustment.
- If the user intended the supplied completion criteria about interactive
  terminal sessions, resize propagation, or exit-code fidelity to be part of
  this same change, stop. Those criteria belong to later terminal-facing work,
  not roadmap 1.1 as written.

## Risks

- The repository currently assumes one binary target in the Makefile. Converting
  to a virtual workspace can break `make build` and `make release` unless those
  targets are rewritten deliberately.
- The CI workflow already delegates part of test execution to a shared coverage
  action. That makes it easy to assume tests are already gated when the exact
  commands are not visible in this repository.
- `docs/users-guide.md` is missing, so the implementation must decide how much
  end-user documentation is appropriate for a scaffolding-only milestone.
- The design document describes later systemd and Qdrant work. It is easy to
  overbuild 1.1 by introducing premature operational abstractions rather than a
  minimal compileable scaffold.
- The project notes store failed during planning. This plan relies on repository
  sources only.

## Milestone 1: Convert the repository into a workspace

Create a virtual workspace at the repository root and move all executable and
library code into member crates under `crates/`. The root `Cargo.toml` becomes
the workspace manifest and central place for shared metadata, shared
dependencies, and shared lint policy. The old root package `repovec_appliance`
is removed.

Create these members:

1. `crates/repovec-core`
2. `crates/repovecd`
3. `crates/repovec-mcpd`
4. `crates/repovec-tui`
5. `crates/repovectl`

Use Cargo package names that match the roadmap names. For the shared library,
the package name should be `repovec-core` and the Rust crate identifier will be
`repovec_core`. Record this binary-name versus crate-name distinction in the
design document so future systemd and documentation work does not drift.

Each binary crate should contain a minimal documented entrypoint and a small
`run()` function that returns `Result<(), ErrorType>` or equivalent, even if
the first implementation is a stub. This avoids growing large `main()`
functions later and makes unit testing easier. `repovec-core` should expose the
smallest useful shared types for this step, such as an application name enum,
shared paths, or configuration stubs, but should not guess at future daemon
logic.

Before moving on, run:

```sh
set -o pipefail
cargo build --workspace 2>&1 | tee /tmp/repovec-1.1-cargo-build.log
```

Success is a zero exit code and five compiled member crates.

## Milestone 2: Rebuild the quality baseline at workspace scope

Move the strict lint baseline from the current root package manifest to
workspace scope so every member crate inherits it. Prefer Cargo workspace lint
inheritance rather than duplicating the lint sections into every crate. Add the
necessary per-crate opt-in so the member crates inherit the workspace lints.

Add `rustfmt.toml` at the repository root with the exact contents below.

```toml
unstable_features = true
comment_width = 100
format_code_in_doc_comments = true
imports_granularity = "Crate"
imports_layout = "HorizontalVertical"
wrap_comments = true
group_imports = "StdExternalCrate"
use_try_shorthand = true
hex_literal_case = "Lower"
format_strings = true
format_macro_matchers = true
fn_single_line = true
condense_wildcard_suffixes = true
use_field_init_shorthand = true
```

Update the Makefile so its build-oriented targets work for a multi-crate
workspace. The key commit gates must stay stable:

1. `make check-fmt`
2. `make lint`
3. `make test`

If `make build` and `make release` still exist after the conversion, make their
behaviour explicit. A reasonable outcome is to build the full workspace instead
of one binary, because there is no single appliance binary after this step.

The implementation must validate the gates with `tee`:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-1.1-check-fmt.log
```

```sh
set -o pipefail
make lint 2>&1 | tee /tmp/repovec-1.1-lint.log
```

```sh
set -o pipefail
make test 2>&1 | tee /tmp/repovec-1.1-test.log
```

Do not proceed to marking roadmap items done until all three commands complete
successfully.

## Milestone 3: Introduce observable scaffold behaviour and tests

Roadmap 1.1 is mostly structural, but the user explicitly asked for both unit
tests and behavioural tests. To satisfy that requirement without inventing
large amounts of premature functionality, give each binary a tiny, explicit,
observable command-line surface.

The recommended shape is:

1. each binary starts successfully with `--help`
2. each binary reports a version with `--version`
3. each binary rejects an unknown argument with a non-zero exit code

Use a thin CLI parser so this behaviour is stable and testable. Keep the
behaviour intentionally small. The purpose is not to design the final command
surface yet; it is to establish a testable contract for the scaffold.

Add unit tests with `rstest` for the shared parsing or configuration pieces in
`repovec-core`. Keep fixtures small and local. Use `#[case]` for happy and
unhappy examples instead of open-ended test matrices.

Add behavioural tests with `rstest-bdd` `0.5.0`. Use compile-time step
validation. Keep scenarios isolated and focused on observable outcomes. A good
minimum set is:

1. happy path: `repovectl --help` exits successfully and shows the binary name
2. unhappy path: `repovectl --definitely-invalid` exits unsuccessfully
3. edge case: one daemon binary with no extra arguments exits successfully in a
   documented stub mode, or prints a controlled "not implemented yet" message

If a binary should not run without configuration yet, the edge-case scenario
may instead assert a stable, intentional failure message. The important point
is that the behaviour is explicit and under test rather than accidental.

Add at least one public API in `repovec-core` with a Rustdoc example that runs
as a doctest. Keep the example realistic, brief, and free of `.unwrap()` and
`.expect()`.

## Milestone 4: Align CI with the commit gates

Update `.github/workflows/ci.yml` so the repository runs the commit-gate
Makefile targets on every push and pull request. Keep the existing Markdown
lint and coverage-related jobs unless they conflict with the new workspace
layout. Add an explicit build step or make the existing test path unambiguously
cover compilation.

The workflow should make it obvious to a reviewer which checks gate the change:

1. `make check-fmt`
2. `make lint`
3. `make test`

If an explicit build step is retained, prefer `cargo build --workspace` or a
Make target that wraps it. If the coverage action already runs tests, do not
hide the gating semantics inside the action alone; keep an explicit `make test`
step in the workflow so the repository-local contract is readable.

Because GitHub branch protection is a repository setting rather than a file,
add a short note to the plan implementation record describing the exact status
checks that must be required in the default branch protection rule. Do not mark
"gate merge on all checks passing" complete unless either:

1. the repository settings were updated and verified, or
2. the roadmap/design document is updated to clarify that the repository change
   is complete but the hosted repository setting remains an operator task

## Milestone 5: Update project documentation and roadmap state

Update `docs/repovec-appliance-technical-design.md` to record the accepted
crate layout decision. At minimum, document that the repository now uses a
virtual workspace root and that the five member crates map to the control
plane, MCP adapter, TUI, provisioning CLI, and shared contracts.

Create `docs/users-guide.md` if it still does not exist. For this milestone the
guide only needs the user-visible behaviour introduced here, such as how to run
`--help` and `--version` for the scaffolded binaries and what those stub
commands do or do not guarantee yet. Keep it honest: this step scaffolds the
programs; it does not deliver the full appliance workflow.

After all validation passes, mark these roadmap items as done in
`docs/roadmap.md`:

1. `1.1.1. Define Cargo workspace with binary crates`
2. `1.1.2. Establish lint and formatting baseline`
3. `1.1.3. Add CI gating pipeline`

Do not mark them done earlier than that.

## Acceptance checks

Run these commands and keep the logs:

```sh
set -o pipefail
cargo build --workspace 2>&1 | tee /tmp/repovec-1.1-cargo-build.log
```

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-1.1-check-fmt.log
```

```sh
set -o pipefail
make lint 2>&1 | tee /tmp/repovec-1.1-lint.log
```

```sh
set -o pipefail
make test 2>&1 | tee /tmp/repovec-1.1-test.log
```

```sh
set -o pipefail
make markdownlint 2>&1 | tee /tmp/repovec-1.1-markdownlint.log
```

```sh
set -o pipefail
make nixie 2>&1 | tee /tmp/repovec-1.1-nixie.log
```

Also run at least one direct smoke test per new binary, for example:

```sh
set -o pipefail
cargo run -p repovectl -- --help 2>&1 | tee /tmp/repovec-1.1-repovectl-help.log
```

```sh
set -o pipefail
cargo run -p repovec-mcpd -- --version 2>&1 | tee /tmp/repovec-1.1-mcpd-version.log
```

Expected results:

1. all commands exit successfully except the intentionally invalid CLI scenario
2. the BDD scenario for an invalid argument proves the failure path and exit
   code are controlled
3. the design document, users guide, and roadmap all reflect the new state

## Controlled delegation plan

Use an agent team during implementation, but keep ownership boundaries clear.

1. Agent A: workspace and Cargo manifest conversion, including shared lint
   inheritance and rustfmt setup
2. Agent B: stub crates, small shared core API, unit tests, doctests, and BDD
   scaffolding
3. Agent C: Makefile, CI workflow, and documentation updates

The lead agent remains responsible for final integration, conflict resolution,
and running the full acceptance checks.

## Progress

- [x] 2026-03-28 00:00 UTC: Reviewed the roadmap, design document, Makefile,
  current Cargo manifest, CI workflow, and the referenced testing guides.
- [x] 2026-03-28 00:00 UTC: Confirmed the repository currently contains a single
  placeholder package rather than the required workspace.
- [x] 2026-03-28 00:00 UTC: Drafted this ExecPlan and captured the major scope
  ambiguities.
- [ ] Await user approval before implementation.
- [ ] Convert the root package into a virtual workspace and add the five member
  crates.
- [ ] Rebuild the lint, formatting, and Makefile baseline at workspace scope.
- [ ] Add unit tests, doctests, and `rstest-bdd` behavioural tests.
- [ ] Update CI to run commit-gate targets on push and pull request.
- [ ] Update the design document, create or update the users guide, and mark
  roadmap item 1.1 done after all gates pass.
- [ ] Record the final outcomes and retrospective.

## Surprises & Discoveries

- The request referenced `docs/podbot-roadmap.md`, but the repository contains
  `docs/roadmap.md` instead.
- `docs/users-guide.md` is currently missing.
- The notes store required by the repository instructions returned tool errors
  during planning, so no project-memory context was available.
- The existing CI workflow is close to the target state but does not trigger on
  `push`.
- The supplied completion criteria about interactive terminal handling do not
  match roadmap 1.1 and appear to belong to later TUI or session-management
  work.

## Decision Log

- 2026-03-28: Treat `docs/roadmap.md` as the authoritative roadmap document for
  this plan because `docs/podbot-roadmap.md` does not exist in the repository.
- 2026-03-28: Convert the repository root into a virtual workspace rather than
  retaining the placeholder `repovec_appliance` package. This matches the
  architecture better and avoids a misleading fifth binary.
- 2026-03-28: Add deliberately small CLI behaviour so the user-requested unit
  tests and BDD tests have something observable to validate without dragging
  later features into 1.1.
- 2026-03-28: Treat GitHub branch protection as a likely external follow-up
  rather than something guaranteed from repository files alone.
- 2026-03-28: Treat the interactive-session completion criteria as out of scope
  for roadmap 1.1 unless the user explicitly broadens the workstream.

## Outcomes & Retrospective

This plan is still in draft. No implementation work has started. After the user
approves the plan and the implementation completes, replace this section with a
concise summary of what shipped, what changed from the draft, what surprised
us, and what follow-up work remains.
