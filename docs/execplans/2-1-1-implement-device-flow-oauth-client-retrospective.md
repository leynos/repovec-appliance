# Device-flow OAuth client retrospective

This artefact preserves execution evidence moved out of the roadmap item
`2.1.1` ExecPlan so the plan remains pre-implementation focused.

## Progress

- [x] (2026-05-26T01:35:00+02:00) Loaded the `leta`,
  `hexagonal-architecture`, `rust-router`, `execplans`, `firecrawl-mcp`,
  `rust-errors`, `rust-async-and-concurrency`, `arch-crate-design`,
  `commit-message`, `pr-creation`, and `en-gb-oxendict-style` skills relevant
  to this planning task.
- [x] (2026-05-26T01:36:00+02:00) Created a `leta` workspace for this
  worktree.
- [x] (2026-05-26T01:37:00+02:00) Confirmed the starting branch was
  `feat/device-flow-execplan` and renamed it to
  `2-1-1-implement-device-flow-oauth-client`.
- [x] (2026-05-26T01:39:00+02:00) Created context pack `pk_kmddzfww` with
  roadmap, technical-design, runtime-path, and testing references for the
  Wyvern planning team.
- [x] (2026-05-26T01:40:00+02:00) Used three Wyvern agents for read-only
  planning briefs covering repository layout, hexagonal architecture/testing,
  and documentation/quality gates.
- [x] (2026-05-26T01:41:00+02:00) Used Firecrawl to resolve external gaps in
  GitHub device-flow behaviour, `octocrab`, the `oauth2` crate,
  `oauth2-test-server`, and `systemd-creds`.
- [x] (2026-05-26T01:45:00+02:00) Drafted this pre-implementation ExecPlan.
- [x] (2026-06-02T00:00:00+02:00) Received explicit approval to proceed with
  implementation from this ExecPlan.
- [x] (2026-06-02T00:01:00+02:00) Confirmed the worktree is clean and on
  branch `2-1-1-implement-device-flow-oauth-client`.
- [x] (2026-06-02T00:15:00+02:00) Ran Milestone 0 baseline gates before code
  changes. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  passed. `make test` reported 183 nextest tests and 29 doctests passing.
  `make markdownlint` also passed for this plan update.
- [x] (2026-06-02T00:40:00+02:00) Added the pure
  `repovec_core::github_oauth` device-flow policy module with redacted secret
  wrappers, OAuth error classification, polling decisions, rstest unit tests,
  and proptest interval invariants. `make check-fmt`, `make typecheck`,
  `make lint`, and `make test` passed. `make test` reported 196 nextest tests
  and 40 doctests passing.
- [x] (2026-06-02T01:14:00+02:00) Ran `coderabbit review --agent` after a
  recoverable rate-limit backoff. CodeRabbit completed with 0 findings.
- [x] (2026-06-02T01:55:00+02:00) Added the `repovecd` OAuth protocol,
  device-flow orchestration, encrypted token-store adapters, behavioural
  scenarios, and `device-flow-test` example binary. `make check-fmt`,
  `make typecheck`, `make lint`, and `make test` passed. `make test` reported
  210 nextest tests and 43 doctests passing.
  `cargo run -p repovecd --example device-flow-test` completed successfully
  against `oauth2-test-server` and loaded a stored token back from the
  encrypted-token adapter boundary.
- [x] (2026-06-02T02:39:00+02:00) Ran `coderabbit review --agent` for the
  adapter/test-binary milestone after a 28 minute rate-limit backoff.
  CodeRabbit reported 9 findings. Valid findings were fixed by adding local
  expiry enforcement, binding `systemd-creds decrypt` to the same credential
  name as encrypt, creating token credential files with owner-only permissions,
  exact example-binary token round-trip validation, endpoint validation at
  client construction, a corrected token URL doc comment, command-argument
  assertions, and documentation that reloaded tokens do not carry scope
  metadata.
- [x] (2026-06-02T02:46:00+02:00) Re-ran deterministic gates after CodeRabbit
  fixes. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  passed. `make test` reported 213 nextest tests and 43 doctests passing.
  `cargo run -p repovecd --example device-flow-test` completed successfully.
- [x] (2026-06-02T03:25:00+02:00) Re-ran CodeRabbit after a 26 minute
  rate-limit backoff. CodeRabbit reported 4 follow-up findings. Fixed the
  still-valid findings by strengthening `systemd-creds` stderr redaction,
  removing a redundant expiry check, and converting local test helpers to
  `rstest` fixtures. The temporary input-file permission recommendation could
  not be implemented with `std::fs::File::set_permissions` because
  `whitaker-lint` forbids ambient `std::fs` operations; the implementation now
  documents the reliance on `tempfile`'s Unix `0600` creation mode instead.
- [x] (2026-06-02T03:32:00+02:00) Re-ran deterministic gates after the
  follow-up fixes. `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` all passed. `make test` reported 213 nextest tests and 43
  doctests passing. `cargo run -p repovecd --example device-flow-test`
  completed successfully.
- [x] (2026-06-02T03:58:00+02:00) Re-ran CodeRabbit after a 23 minute
  rate-limit backoff. CodeRabbit reported 3 follow-up findings. Fixed them by
  expanding the `repovecd` crate-level documentation, redacting all known
  GitHub token prefixes in `systemd-creds` stderr display, adding parameterized
  redaction tests, and removing the intermediate allocation from scope joining.
- [x] (2026-06-02T04:04:00+02:00) Re-ran deterministic gates after the latest
  follow-up fixes. `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` all passed. `make test` reported 218 nextest tests and 43
  doctests passing. `cargo run -p repovecd --example device-flow-test`
  completed successfully.
- [x] (2026-06-02T05:05:00+02:00) Re-ran CodeRabbit after a 30 minute
  rate-limit backoff. CodeRabbit reported 3 follow-up findings. Fixed the
  still-valid findings by removing the redundant zero-duration expiry check and
  adding the missing `ghu_` GitHub OAuth token prefix to stderr redaction and
  tests. The suggested `rstest` fixture simplification is intentionally not
  applied because the direct default-return form fails `make typecheck` with an
  `unused_braces` diagnostic, and the mechanical workarounds fail Clippy.
- [x] (2026-06-02T05:18:00+02:00) Re-ran deterministic gates after those
  fixes. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  passed. `make test` reported 219 nextest tests and 43 doctests passing.
  `cargo run -p repovecd --example device-flow-test` completed successfully.
- [x] (2026-06-02T06:21:00+02:00) Re-ran CodeRabbit after two requested
  rate-limit backoffs. CodeRabbit reported 5 findings. Fixed the valid issues
  by parsing token-poll response bodies before assuming success status,
  validating both endpoint URLs with `Url::parse`, fsyncing the containing
  directory by opening `.` after atomic rename, removing a now-unused
  `systemd-creds` error variant, and letting the top-of-loop expiry guard own
  the next-interval check.
- [x] (2026-06-02T06:35:00+02:00) Re-ran deterministic gates after these
  fixes. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  passed. `make test` reported 219 nextest tests and 43 doctests passing.
  `cargo run -p repovecd --example device-flow-test` completed successfully.
- [x] (2026-06-02T06:52:00+02:00) Re-ran CodeRabbit after the latest fixes.
  CodeRabbit completed with 0 findings, clearing the adapter/test-binary
  milestone for commit and documentation work.
- [x] (2026-06-02T06:55:00+02:00) Committed the adapter/test-binary milestone
  as `0fdfa18`.
- [x] (2026-06-02T07:05:00+02:00) Updated the technical design, users guide,
  developers guide, and roadmap. The design now records the `oauth2` adapter
  decision and encrypted-token persistence contract; the guides document
  operator-visible login behaviour and internal boundary conventions; roadmap
  item `2.1.1` is marked done.
- [x] (2026-06-02T07:15:00+02:00) Ran documentation and code gates after the
  documentation closeout. `make markdownlint`, `make nixie`, `make check-fmt`,
  `make typecheck`, `make lint`, and `make test` all passed. `make test`
  reported 219 nextest tests and 43 doctests passing. `make fmt` was attempted,
  but the repository-wide formatter target exits non-zero on pre-existing
  unrelated Markdown lint issues; incidental formatter changes to unrelated
  files were restored before continuing.
- [x] (2026-06-02T07:26:00+02:00) Ran CodeRabbit for the documentation and
  roadmap closeout. CodeRabbit completed with 0 findings.
- [x] (2026-06-24T00:00:00+02:00) Began review-feedback pass for post-review
  security, orchestration, and documentation findings. Verified the branch,
  reset Lody session title, created a Leta workspace, and patched still-valid
  issues while keeping the changes focused.
- [x] (2026-06-24T00:00:00+02:00) Validated the review-feedback fixes.
  `make build`, `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` passed. `make test` reported 258
  nextest tests and 45 doctests passing.
  `cargo run -p repovecd --example device-flow-test` completed successfully.
