# Developers guide

This guide is for maintainers and contributors working on repovec-appliance. It
describes the repository-level build, test, lint, and continuous integration
(CI) workflow that must remain true as the project evolves.

## Normative references

- [Documentation contents](contents.md) if present
- [repovec-appliance technical design](repovec-appliance-technical-design.md)
- [Roadmap](roadmap.md)
- [Execution plan for 1.1.3: CI gating pipeline](execplans/1-1-3-ci-gating-pipeline.md)

## Local quality gates

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

When a change touches Markdown, also run:

```sh
set -o pipefail
make markdownlint 2>&1 | tee /tmp/repovec-make-markdownlint.log
set -o pipefail
make nixie 2>&1 | tee /tmp/repovec-make-nixie.log
```

These Make targets are the source of truth for local validation and for CI. Do
not duplicate or partially reimplement them in workflow YAML.

## GitHub Actions gate set

The repository CI workflow exposes five stable, required job names:

- `build`
- `check-fmt`
- `lint`
- `test`
- `docs-gate`

The first four jobs run on every push, pull request update, and manual workflow
dispatch.

`docs-gate` always reports a result so it can be configured as a required
check. It runs `make markdownlint` and `make nixie` only when the changed-file
set includes a documentation input. Markdown inputs use one of these extensions:

- `.md`
- `.markdown`
- `.mdx`

Documentation-tooling configuration changes also count as documentation input:

- `.markdownlint-cli2.jsonc`

When the changed-file list is unavailable, the workflow runs the documentation
gate conservatively instead of risking a skipped validation.

`make nixie` is narrower than `make markdownlint`: Mermaid validation runs only
when one of the changed Markdown files contains a Mermaid diagram, or when a
documentation-tooling configuration change requires the conservative path. The
user-visible flow is documented in [users-guide.md](users-guide.md).

## CI policy helper

The Markdown change classification logic lives in the `repovec-ci` crate. Keep
that helper small and policy-focused.

- Unit coverage uses `rstest`.
- Behavioural coverage uses `rstest-bdd` with
  `strict-compile-time-validation`.
- Workflow shell should stay thin and delegate any classification logic to the
  helper rather than embedding untestable conditions directly in YAML.
- The helper emits conservative-fallback fields so workflow logs can
  distinguish an actual Mermaid match from an unreadable file that forced
  `make nixie` to run.

## Required-check enforcement

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
