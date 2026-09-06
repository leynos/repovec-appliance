# Architectural decision record (ADR) 001: Retire the custom OAuth device flow

## Status

Proposed. Merging the pull request that introduces this record accepts the
migration direction. Delivery remains tracked separately in the roadmap.

## Date

2026-08-29.

## Context and problem statement

`repovec-core` and `repovecd` currently implement substantial parts of the
OAuth 2.0 device authorization grant themselves. The implementation defines
protocol-facing device-code, user-code, token, polling-outcome, and terminal
error types, parses the wire responses, maintains the active polling interval,
and applies `authorization_pending`, `slow_down`, expiry, and completion
semantics.

The workspace already depends on the [`oauth2`](https://docs.rs/oauth2/) crate.
That crate supplies device authorization and device access-token exchanges,
including polling behaviour. Keeping a second RFC 8628 implementation creates a
security-sensitive compatibility surface that repovec does not need to own.

The existing code has useful application policy and test seams, but protocol
mechanics and application policy are currently interleaved.

## Decision drivers

- OAuth protocol correctness should come from a maintained specialist crate.
- GitHub-specific endpoint configuration must remain explicit and testable.
- Secret values must remain redacted from `Debug`, errors, and telemetry.
- Polling tests must stay deterministic and must not sleep in wall-clock time.
- Existing operator prompts, metrics, and encrypted persistence must survive the
  migration.
- The migration must not silently change GitHub scope or expiry handling.

## Options considered

### Retain and complete the current implementation

This preserves all existing seams but leaves repovec responsible for RFC 8628
wire compatibility and edge cases. It duplicates an existing dependency and
increases the audit surface.

### Wrap the `oauth2` device-flow implementation

This delegates protocol state and response parsing to `oauth2`. A thin repovec
adapter retains endpoint selection, HTTP client construction, prompt
presentation, telemetry, and token persistence.

### Replace the flow with direct GitHub API calls in the TUI

This would move protocol ownership rather than remove it, and it would couple
operator presentation to GitHub transport behaviour.

## Decision outcome / proposed direction

The project will use the `oauth2` crate as the sole implementation of the OAuth
2.0 device authorization grant.

The replacement adapter will:

- construct a stateful `oauth2` client with GitHub's device-code and token
  endpoints;
- request a device authorization through `oauth2`'s device authorization API;
- poll through `oauth2`'s device access-token API;
- use one shared, redirect-disabled HTTP client with explicit connect and
  request timeouts;
- map library errors into a small repovec application error taxonomy;
- present the verification URI and user code through the existing presentation
  port;
- retain repovec-owned tracing, metrics, and encrypted token persistence; and
- preserve deterministic tests through the narrowest clock or sleeper adapter
  supported by the library.

The following current machinery will be deleted after behavioural parity is
proved:

- custom device-flow wire response structures;
- duplicate protocol error-code classification;
- `TokenPollOutcome` and `PollDecision` when they merely mirror `oauth2`;
- the handwritten active polling state machine; and
- direct construction of token endpoint requests.

Repovec-specific types may remain only where they enforce an application
invariant that the upstream type does not express. Such wrappers must not copy
OAuth protocol semantics.

## Goals and non-goals

Goals:

- remove duplicate RFC 8628 implementation code;
- retain observable, deterministic application behaviour;
- reduce the amount of secret-bearing code maintained by the project; and
- make future `oauth2` updates ordinary dependency upgrades.

Non-goals:

- replacing GitHub repository API access;
- changing the operator login interaction;
- introducing refresh tokens where GitHub does not issue them; or
- changing token-at-rest policy, which is governed by ADR 005.

## Migration plan

1. Freeze the current behaviour with tests covering prompts, intervals,
   `slow_down`, expiry, denial, scopes, redaction, and persistence boundaries.
2. Build a new adapter around `oauth2` and `oauth2-test-server` without changing
   the public operator workflow.
3. Run the old and new implementations against the same deterministic scenario
   table and reconcile intentional differences.
4. Switch production composition to the upstream-backed adapter.
5. Delete the duplicate wire and polling implementation, then update the design
   document and migration notes.
6. Add a dependency-boundary test that rejects reintroduction of handwritten
   OAuth token endpoint requests outside the adapter.

## Known risks and limitations

- `oauth2` may expose different error granularity from the current code. The
  adapter must avoid rebuilding its protocol taxonomy under new names.
- Deterministic timing tests may require a small upstream contribution if the
  available sleeper or clock seam proves insufficient.
- A dependency upgrade can change polling behaviour. Scenario tests and
  changelog review remain mandatory.

## Architectural rationale

This decision keeps protocol mechanics at the infrastructure boundary and
retains only repovec policy in the application layer. It removes code rather
than turning accidental duplication into another internal framework.
