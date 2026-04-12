# Execution plan for 1.1.3: CI gating pipeline

## Preamble

- Status: In progress
- Roadmap item: `1.1.3`
- Last updated: 2026-04-12
- Primary references:
  - [docs/roadmap.md](../roadmap.md)
  - [docs/repovec-appliance-technical-design.md](../repovec-appliance-technical-design.md)
  - [docs/rust-testing-with-rstest-fixtures.md](../rust-testing-with-rstest-fixtures.md)
  - [docs/rstest-bdd-users-guide.md](../rstest-bdd-users-guide.md)
  - [docs/rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md)
  - [docs/reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md)
  - [docs/complexity-antipatterns-and-refactoring-strategies.md](../complexity-antipatterns-and-refactoring-strategies.md)
  - [docs/ortho-config-users-guide.md](../ortho-config-users-guide.md)
  - [docs/documentation-style-guide.md](../documentation-style-guide.md)

## Summary

Task `1.1.3` is now implemented in-repository but not yet complete at the
repository-settings layer. The GitHub Actions workflow has been rewritten to
run the full Make-based gate set on every push, documentation gates now run
conditionally through a tested helper, and the required-check ruleset payload
is versioned in the repository.

Completion still has two layers:

1. align the repository workflow with the Make targets that define the commit
   gates; and
2. enforce those checks as merge blockers through branch protection or an
   equivalent GitHub ruleset.

The roadmap entry should remain pending until both layers are in place on the
remote repository.

## Current state

- [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) now runs on
  every push, relevant pull request events, and manual dispatch.
- The workflow exposes stable required job names: `build`, `check-fmt`,
  `lint`, `test`, and `docs-gate`.
- `docs-gate` computes changed files in-workflow and runs
  `make markdownlint` when Markdown files changed, while `make nixie` runs only
  when a changed Markdown file contains a Mermaid diagram. The job still
  publishes a stable required check result.
- The docs-gate classification logic now lives in the `repovec-ci` crate and is
  covered by `rstest` unit tests plus `rstest-bdd` behavioural tests.
- The desired GitHub ruleset is versioned in
  [`.github/rulesets/main-ci-gating.json`](../../.github/rulesets/main-ci-gating.json).
- Live enforcement is still pending because the ruleset has not been activated
  on the remote repository yet.

## Delivery goals

The implementation is complete when all of the following are true:

- every push triggers the core CI gate set:
  `make build`, `make check-fmt`, `make lint`, and `make test`
- documentation changes additionally trigger `make markdownlint` and
  `make nixie`
- pull requests expose stable check names that can be marked as required
- merges to the protected branch are blocked until all required checks pass
- the design and contributor-facing documentation explain the chosen
  enforcement model
- the roadmap item `1.1.3` is marked done only after the enforcement has been
  verified

## Constraints and decisions

### Make targets are the source of truth

The repository instructions require Make targets to define commit gates, so the
workflow should invoke `make build`, `make check-fmt`, `make lint`,
`make test`, `make markdownlint`, and `make nixie` rather than duplicating
their underlying commands in YAML.

### Branch protection is an external dependency

Workflow changes alone cannot complete this roadmap item. The merge gate is not
real until GitHub branch protection or a GitHub ruleset requires the workflow
checks. The implementation therefore needs one explicit enforcement path:

- manage the rule manually in the GitHub UI and document the exact required
  checks; or
- manage it through repository automation if the project already has an
  accepted pattern for GitHub settings management.

If no repository-managed automation exists, the roadmap must stay pending until
the manual configuration has been applied and verified.

Implementation update, 2026-04-12:

- `gh` authentication is available in the implementation environment.
- The repository currently has no existing rulesets.
- The implementation will use a GitHub repository ruleset targeting
  `refs/heads/main` rather than leaving merge enforcement as a manual-only
  follow-up.

### Test policy should live in versioned code

The user requirement calls for unit tests with `rstest` and behavioural tests
with `rstest-bdd` v0.5.0. Pure YAML is difficult to validate that way, so the
implementation should avoid burying CI policy entirely inside the workflow. The
preferred approach is to extract the policy that decides whether the
documentation gates should run into a small Rust helper with a narrow surface,
then keep the workflow as a thin adapter around that helper.

That approach gives the project a place to add:

- `rstest` unit tests for change classification
- `rstest-bdd` scenarios for happy and unhappy paths
- deterministic validation without relying on GitHub-hosted runners

Implementation update, 2026-04-12:

- The helper will live in a dedicated workspace crate, `repovec-ci`, instead of
  inside `repovec-core`.
- `docs-gate` will always publish a stable job result so it can be configured as
  a required check even when no Markdown files changed.

## Workstreams

### 1. Define the enforcement model

Document how required checks will be enforced before editing the workflow.

Planned steps:

1. confirm whether branch protection will be managed manually or through
   automation
2. choose the exact check names that branch protection will require
3. record that choice in the design document or an ADR if the policy is meant
   to be durable

Exit criteria:

- one documented enforcement path exists
- required check names are fixed and stable

Progress, 2026-04-12:

- Enforcement path selected: GitHub repository ruleset applied through
  `gh api`.
- Planned required checks:
  - `build`
  - `check-fmt`
  - `lint`
  - `test`
  - `docs-gate`
- A versioned ruleset payload will be committed in-repo, but live activation is
  deferred until the new workflow exists on the default branch. Enabling the
  ruleset against the current remote state would require check names that do
  not yet exist there.
- The desired ruleset payload is now stored in
  [`.github/rulesets/main-ci-gating.json`](../../.github/rulesets/main-ci-gating.json).

### 2. Refactor CI into explicit gate jobs

Update the existing workflow so that it matches the roadmap item directly.

Planned steps:

1. change the `push` trigger so it runs on every push rather than only on
   `main`
2. keep pull request coverage and manual dispatch support
3. split the workflow into clearly named jobs so each required check is easy to
   reason about
4. preserve the Rust toolchain setup, Whitaker installation, cache behaviour,
   and pinned action SHAs already used by the repository
5. replace the direct Markdown action with `make markdownlint`
6. add `make nixie` behind documentation change detection
7. replace or supplement the coverage-specific test path with an explicit
   `make test`

Recommended job layout:

- `build`
- `check-fmt`
- `lint`
- `test`
- `docs-gate`

If coverage upload remains valuable, keep it as a non-blocking or separate job
that does not replace the required `test` gate.

Exit criteria:

- the workflow names match the planned required checks
- every core gate runs on every push
- the documentation gate only runs when documentation changes

Progress, 2026-04-12:

- The existing monolithic `build-test` job will be split into separate required
  jobs with those stable names.
- The existing coverage upload path will be dropped from the required gate set
  so `make test` becomes the authoritative test gate.
- `docs-gate` will compute the changed-file set inside the workflow and always
  publish a required status, running `make markdownlint` when Markdown files
  changed and `make nixie` only when a changed Markdown file contains a Mermaid
  diagram.
- The hosted runner did not provide `markdownlint-cli2`, so `docs-gate` now
  installs `markdownlint-cli2@0.22.0` before invoking `make markdownlint`.
- The rewritten workflow is now in place at
  [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml).

### 3. Introduce a testable CI-policy helper

Factor the decision logic for documentation-gate execution into a small Rust
module or binary so the policy can be exercised through ordinary tests.

Suggested responsibilities:

- accept a changed-file list as input
- classify whether documentation gates must run
- expose a stable output that the workflow can consume

Unit-test plan with `rstest`:

- docs-only change set triggers docs gates
- code-only change set skips docs gates
- mixed code and docs change set triggers docs gates
- non-documentation asset changes do not produce false positives

Behavioural test plan with `rstest-bdd` v0.5.0:

- happy path: a push containing Markdown changes schedules both the core gates
  and the docs gate
- happy path: a push containing only Rust changes schedules only the core gates
- unhappy path: an empty or malformed changed-file list falls back to the safe
  behaviour chosen by the implementation
- edge case: Mermaid-bearing documentation changes trigger `make nixie`

Implementation note:

The helper should stay small and single-purpose. Do not build a large internal
CI framework around one roadmap item.

Progress, 2026-04-12:

- The helper will classify Markdown changes globally by file extension
  (`.md`, `.markdown`, and `.mdx`) so README and other repository documentation
  updates are covered alongside `docs/`.
- An empty or unavailable changed-file list will conservatively run the docs
  gate.
- Mermaid validation is now a separate output flag so the workflow can skip
  `make nixie` when Markdown changed but no changed Markdown file contains a
  Mermaid block.
- The helper crate and its tests are now implemented under
  [`crates/repovec-ci/`](../../crates/repovec-ci/).

### 4. Update documentation

This work is maintainer-facing, so documentation changes should focus on the
design and contributor workflow.

Planned steps:

1. update
   [docs/repovec-appliance-technical-design.md](../repovec-appliance-technical-design.md)
    if the repository wants the CI enforcement model captured as part of the
   system design
2. add or update maintainer-facing documentation describing:
   - the CI gate set
   - when documentation gates run
   - how required checks are enforced
   - how to update required check names safely
3. review whether `docs/users-guide.md` needs an update

Current expectation:

- this task does not appear to introduce an operator-facing behaviour change,
  so a users guide update is likely unnecessary
- if the implementation does introduce user-visible workflow behaviour, create
  or update `docs/users-guide.md` accordingly

Exit criteria:

- maintainers can see how CI is expected to behave
- any durable design decision is recorded in the design document or an ADR

Progress, 2026-04-12:

- Planned documentation updates:
  - add maintainer guidance in `docs/developers-guide.md`
  - record the CI and ruleset decision in
    `docs/repovec-appliance-technical-design.md`
  - keep `docs/users-guide.md` unchanged unless implementation introduces a
    user-facing behaviour change
- add `docs/contents.md` so the documentation set indexes the new developers
  guide and execution plan
- Those documentation updates are now in place, and `docs/users-guide.md`
  now documents the docs-gate decision flow with an accessible Mermaid diagram.

### 5. Verify locally and in GitHub

Local validation must use the repository gate commands and preserve exit codes.

Planned local checks:

```sh
set -o pipefail
make build 2>&1 | tee /tmp/repovec-make-build.log
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-make-check-fmt.log
set -o pipefail
make lint 2>&1 | tee /tmp/repovec-make-lint.log
set -o pipefail
make test 2>&1 | tee /tmp/repovec-make-test.log
set -o pipefail
make markdownlint 2>&1 | tee /tmp/repovec-make-markdownlint.log
set -o pipefail
make nixie 2>&1 | tee /tmp/repovec-make-nixie.log
```

Remote verification steps:

1. open a pull request that changes code only and confirm only the core gates
   are required
2. open a pull request that changes documentation and confirm the docs gate
   runs and is required
3. confirm a failing required check blocks merge
4. confirm the branch protection or ruleset points at the final job names

Only after that verification should the roadmap item be marked done.

Implementation progress, 2026-04-12:

- Local validation completed successfully:
  - `make build`
  - `make check-fmt`
  - `make lint`
  - `make test`
  - `make markdownlint`
  - `make nixie`
- Remote verification and ruleset activation remain outstanding.

### GitHub ruleset deployment commands

Run these commands only after the workflow changes in
[`/.github/workflows/ci.yml`](../../.github/workflows/ci.yml) are present on
the remote default branch.

Inspect the current rulesets:

```sh
gh api repos/leynos/repovec-appliance/rulesets
```

Create the ruleset if it does not exist yet:

```sh
gh api \
  --method POST \
  repos/leynos/repovec-appliance/rulesets \
  --input .github/rulesets/main-ci-gating.json
```

Update the existing ruleset if one named `main-ci-gating` already exists:

```sh
RULESET_ID="$(gh api repos/leynos/repovec-appliance/rulesets --jq '.[] | select(.name == "main-ci-gating") | .id')"
gh api \
  --method PUT \
  "repos/leynos/repovec-appliance/rulesets/${RULESET_ID}" \
  --input .github/rulesets/main-ci-gating.json
```

Verify that the required checks match the workflow job names:

```sh
gh api repos/leynos/repovec-appliance/rulesets --jq '.[] | select(.name == "main-ci-gating")'
```

## Risks and mitigations

### GitHub enforcement may remain out of band

Risk: The repository may not have an accepted automation path for branch
protection.

Mitigation: Document the manual GitHub configuration precisely and keep the
roadmap entry pending until it has been applied.

### The current coverage job may mask the missing test gate

Risk: Keeping only a coverage-oriented test path would not satisfy the roadmap
wording or the repository's Make-target policy.

Mitigation: Make `test` a first-class required job. Treat coverage upload as
secondary.

### YAML-only logic is hard to test with Rust

Risk: The requested `rstest` and `rstest-bdd` coverage becomes artificial if
all policy remains inside GitHub Actions YAML.

Mitigation: Move the path-classification policy into a small Rust helper and
keep the workflow declarative.

### Prompt completion criteria do not match the roadmap item

Risk: The prompt mentions interactive terminal handling, resize propagation,
and exit code reporting, which do not appear related to task `1.1.3`.

Mitigation: Treat those criteria as out of scope for this item unless the user
confirms they are intentional cross-cutting requirements.

## Definition of done

Mark roadmap item `1.1.3` as done only when:

- the workflow is merged
- the required checks from
  [`.github/rulesets/main-ci-gating.json`](../../.github/rulesets/main-ci-gating.json)
   are configured and verified in GitHub
- the local Make gates pass
- the relevant documentation updates are merged
- the final pull request shows the required checks blocking merge as intended
