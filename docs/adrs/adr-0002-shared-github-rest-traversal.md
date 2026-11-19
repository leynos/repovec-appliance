# Architectural decision record (ADR) 0002: Share one confined traversal for GitHub REST listings

## Status

Accepted.

## Date

2026-11-19.

## Context and problem statement

Roadmap item `2.2.1` requires repository listing through the GitHub REST API:
repositories visible to an authenticated user or to a GitHub App installation,
across all pages, with correct pagination. Roadmap item `2.2.2` then requires
branch listing and open pull request listing for each repository, and both use
the same GitHub pagination protocol.

GitHub paginates with a `Link` response header whose `next` link carries the
continuation URL. A client that follows that URL without checking it can be
sent to another origin or path, can loop forever on a repeated page, and can
accumulate unbounded records, response bodies, or header values. Existing
third-party clients examined during planning did not provide the combination of
continuation confinement, cycle detection, and body, page, and record budgets
that this boundary needs. Writing the same checks per endpoint would mean
either duplicating them or sharing none of them.

The appliance also needs the traversal to remain finite and all-or-nothing: a
failed later page must never look like a complete inventory in which
repositories have disappeared.

## Decision Drivers

- One protocol: every planned listing consumer uses GitHub's `Link` pagination
  over a paginated GET.
- Fail-closed safety: continuation confinement, cycle detection, and budgets
  must apply to every listing, not only the first one implemented.
- Finite work by construction: the appliance must not run an unbounded scan
  against a hostile or malfunctioning endpoint.
- All-or-nothing results: a failure must return an error and no partial list.
- Small, reviewable surface: no speculative abstraction for a protocol no
  consumer uses.
- Reuse in roadmap item `2.2.2` without new traversal logic.

## Options considered

### Option A: One endpoint-specific loop per listing

Each listing module implements its own request loop, Link parsing, and limits.
It duplicates the safety logic and invites divergence, and a defect fixed in
one loop does not protect the others.

### Option B: A public pagination trait or generic pagination engine

Define a trait that each endpoint implements, or adopt a general pagination
crate. Every known consumer shares the same protocol, so a trait would spread
the same continuation logic across implementations while sharing none of the
safety checks. A general engine would still require the confinement, cycle, and
budget policy to be written on top.

### Option C: One crate-private traversal driven by an endpoint profile

A single crate-private traversal in `repovecd` is generic over the item type.
Each endpoint supplies a profile: its path, the query fixed for the whole
traversal, and whether a page is a bare array or an object holding the items
under a named key. The traversal owns Link parsing, `next` selection,
continuation confinement, cycle detection, and every budget.

## Decision outcome

Adopt Option C. Repository listing supplies two profiles: `/user/repos` with
the fixed affiliation, sort, direction, and `per_page=100` query and a
bare-array body, and `/installation/repositories` with `per_page=100` and an
envelope holding `repositories`. Branch and pull request listing add profiles
without new traversal code.

The traversal is crate-private. Integration tests reach a third, non-repository
profile through the `test-support` Cargo feature, which re-exports
crate-private seams only and is enabled by a self dev-dependency. It never
widens the production API.

Link parsing uses `nom-rfc8288`, which returns all link-values and parameters,
so ambiguous or duplicate `next` targets can be rejected rather than guessed.
Selection, confinement, and budgets remain repository code. Redirects are
disabled so a request cannot move without the confinement checks seeing it.

The traversal owns no retries, backoff, scheduling, or whole-scan budget.
Callers own credential acquisition and output; a later workstream owns
reconciliation.

## Goals and non-goals

- Goals:
  - one confined, bounded traversal shared by every GitHub REST listing;
  - typed, all-or-nothing results with stable error categories;
  - reuse for branch and pull request listing without new traversal code.
- Non-goals:
  - GitHub App installation-token issuance or refresh;
  - retries, backoff, or request scheduling;
  - reconciliation policy over the observed repositories;
  - a public extension point for non-GitHub pagination protocols.

## Consequences

- A new listing costs one wire record and one profile constant; the safety
  checks are inherited.
- Changing a budget or a confinement rule changes every listing at once, which
  is the intent.
- The traversal's `test-support` re-exports are test-only surface and must not
  grow into production API.
- Because results are observations rather than a snapshot, consumers must
  tolerate duplicates and misses under concurrent access changes.

## Known risks and limitations

- A traversal is not an atomic remote snapshot. Access changes during paging
  can produce a missed or repeated observation; reconciliation policy owns that
  problem.
- Budgets bound bytes read and records retained, not resident memory in total:
  allocator overhead, decoded records, and retained continuation identities sit
  alongside the byte budgets.
- `nom-rfc8288` parses the field grammar; the repository still owns every
  decision about which link is acceptable.

## Architectural rationale

The appliance's existing boundary rule puts policy in `repovec-core` and
runtime adapters in `repovecd`. Pagination safety is adapter behaviour that
every listing consumer needs identically, so it belongs in one crate-private
component rather than in each endpoint or in a public abstraction. A profile
keeps the only genuine variation (path, fixed query, item location) as data,
and the traversal keeps the security-relevant decisions in one reviewed place.
