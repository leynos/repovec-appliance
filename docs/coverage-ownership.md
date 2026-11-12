# Coverage ownership

Coverage is measured in two workflows, and each measurement has one owner. Pull
requests measure for their own ratchet and publish nothing. A push to `main`
measures once, refreshes the ratchet baseline and uploads the report to
CodeScene. This document records which workflow owns which output, and which
contract clause refuses each way the split could quietly rot.

## Pull-request measurement

`ci.yml` defines the `coverage` job. The workflow also answers a push to
`main`, so the job is guarded with exactly
`if: github.event_name == 'pull_request'`: on a push, the publisher owns the
measurement and running the lane as well would measure the same commit twice.

The job installs the pinned build tools before anything compiles, then calls
the shared `generate-coverage` action with the same `output-path`, `format`,
`language`, `all-targets`, `all-features`, `with-ratchet` and
`publish-artefact` values as the publisher. It uploads no artefact and contains
no CodeScene host, client, uploader, token reference or `codescene`
environment. Its job permissions are `contents: read`.

## The single publisher

`coverage-main.yml` is the only workflow that uploads coverage. It answers
`push: branches: [main]` and nothing else.

- Its concurrency group is `${{ github.workflow }}-${{ github.ref }}` with
  `cancel-in-progress: false`, declared once at workflow scope. A cancelled run
  abandons its upload and its baseline write, and a group repeated on a job is
  a deadlock GitHub resolves by cancelling the job.
- The measuring job runs on a GitHub-hosted runner and sets
  `publish-baseline: never` with a `baseline-artefact-name`. A job that needs it
  (`coverage-baseline`, on Ubicloud) runs `publish-coverage-baseline` on that
  artefact and the same baseline path, with no condition and no
  `continue-on-error`; `coverage-warmup` compiles build-only and is not a
  coverage lane.
- The upload job declares `permissions: contents: read`, checks out with
  `persist-credentials: false`, and installs the build tools before the
  coverage action runs the suite.
- The upload is guarded on the token presence output and on
  `github.ref == 'refs/heads/main'`, and passes the token only as
  `with: access-token`. The presence check runs one command that Actions
  expands to `available=true` or `available=false`; no `env` anywhere binds the
  token, because the uploader is a composite action whose step `env` would
  reach its nested steps.

## The environment

The token lives in the `codescene` environment, whose deployment policy admits
`main` alone. Only the uploading job declares that environment. No other job
declares it, and no workflow a pull request can start declares it anywhere,
because a declaration there would let branch code request a main-only token.

## Verified operational gaps

The Dependabot automerge workflow merges with `GITHUB_TOKEN`, and such a merge
fires no `push` event. Those commits therefore reach the publisher only at the
next ordinary push, so the ratchet baseline and the CodeScene report advance on
push, not on merge. A manual "Re-run jobs" keeps its `run_id` and republishes
that commit's coverage, but replaces no baseline unless the original run saved
none. This is a deliberate limit of the `push`-only trigger, not a reason to add
`workflow_dispatch`: a dispatch cannot be confined to main by the trigger
itself, and the upload's ref guard would then be the only boundary.

### Instrumented fixture builds

`repovec-core`'s `trybuild` driver compiles three UI fixtures, and each fixture
build rebuilds the crate and its dependencies. Under instrumentation that build
outlasts nextest's default budget of three 60-second periods, so the coverage
run reports a timeout instead of the compile-failure assertion the test exists
to make (measured on PR #216, run 41348216522: 627 passed, 1 timed out at 180
s). `.config/nextest.toml` therefore allows that driver ten minutes, selected
by its exact test name, and `nextest_budget_test.py` holds both the budget and
the exactness of the selection. Skipping the driver in the coverage lane would
be simpler and wrong: it is the only check that the opaque capability wrappers
stay opaque.

Evidence limits, recorded so that no later reader mistakes a skipped step for
proof:

- the `codescene` environment exists with a branch policy, but no
  `CS_ACCESS_TOKEN` is visible to the account used for this work;
- the upload being skipped because the presence check wrote `available=false`
  is not evidence that the secret is provisioned or missing;
- the publisher cannot be exercised before it is merged, because its trigger
  is a push to `main` on the merged repository.

Do not claim the secret exists, that CodeScene received a report, or that the
baseline has advanced until a merged push has run the publisher and its log
shows the upload step running.

## Contracts

The rules read parsed workflows through `workflow_loader.py` and
`coverage_workflow_reader.py` in `tests/workflow_contracts`; run them with
`make test-workflow-contracts`.

| Concern                                     | Module                           |
| ------------------------------------------- | -------------------------------- |
| Strict reading and refusals                 | `coverage_workflow_reader.py`    |
| Token placement and presence check          | `coverage_token_rules.py`        |
| Publisher trigger, concurrency, guard, pins | `coverage_publisher_rules.py`    |
| Pull-request closure and CodeScene contacts | `coverage_pull_request_rules.py` |
| Measurement parity and baseline writers     | `coverage_input_rules.py`        |
| `codescene` environment placement           | `coverage_environment_rules.py`  |
| Instrumented `trybuild` timeout budget      | `nextest_budget_test.py`         |

## Mutation proof

Each mutation was applied alone, the named contract was observed to fail, and
the mutation was reverted.

| Mutation                                                                  | Failing contract                                              |
| ------------------------------------------------------------------------- | ------------------------------------------------------------- |
| Token reference added to a pull-request lane                              | `pull_request_contacts`: puts CS_ACCESS_TOKEN in reach        |
| Indirect call to a workflow reaching CodeScene                            | `pull_request_contacts`: names the CodeScene host             |
| Duplicate mapping key in a workflow                                       | strict loader: the document is refused as unreadable          |
| Presence check removed                                                    | `token_violations`: needs one `codescene-token`               |
| Presence check moved after the upload                                     | `token_violations`: must run before the upload                |
| Token added to a step `env`                                               | `token_violations`: must not bind CS_ACCESS_TOKEN             |
| Upload guard widened with `\|\| github.event_name == 'workflow_dispatch'` | `publisher_violations`: upload must be guarded                |
| `cancel-in-progress: true`                                                | `publisher_violations`: cancels a publisher run in progress   |
| A second upload step                                                      | `publisher_violations`: expected one CodeScene upload step    |
| `environment` removed from the upload job                                 | `environment_violations`: uploading job must declare it       |
| `environment` moved to a pull-request job                                 | `environment_violations`: reachable from a pull request       |
| Empty workflow set                                                        | reader: no workflows were read                                |
| Coverage input drift between the lanes                                    | `coverage_violations`: selection differs from the publisher's |
| `terminate-after` returned to the measured failing value                  | `nextest_budget`: budget does not exceed the default          |
| Override filter broadened to `all()`                                      | `nextest_budget`: must select `test(=…)` exactly              |
