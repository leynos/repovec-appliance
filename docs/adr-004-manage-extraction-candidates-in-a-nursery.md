# Architectural decision record (ADR) 004: Manage extraction candidates in a nursery

## Status

Proposed. Merging the pull request that introduces this record accepts the
nursery governance model and the initial three crate boundaries.

## Date

2026-08-29.

## Context and problem statement

Several repovec components contain capabilities that may be useful beyond the
appliance. Immediate publication would force names, semantic versioning, support
expectations, and compatibility promises before the project has proved the
boundaries with more than one implementation or consumer.

Keeping all reusable code inside large production crates has the opposite
problem. Dependencies and side effects accrete until later extraction requires a
large rewrite.

A controlled incubation stage is needed between private helper modules and
public ecosystem crates.

## Decision drivers

- Potentially reusable interfaces should be exercised by real code early.
- Nursery APIs must be free to break while evidence accumulates.
- Production builds must not accidentally depend on speculative crates.
- Candidate crates must not duplicate mature upstream libraries.
- Publication requires a deliberate review rather than a manifest flag flip.
- Abandoned experiments must be easy to delete.

## Decision outcome / proposed direction

The repository will maintain a nested Rust workspace under `crates/nursery/`.
The root production workspace explicitly excludes that directory. Nursery
crates are compiled, linted, tested, and documented by a separate workflow.
Every nursery package sets `publish = false`.

The initial nursery contains three crates:

| Crate | Responsibility | Explicit exclusion |
| --- | --- | --- |
| `repovec-unit-contract` | Parser-neutral rules and structured diagnostics | Parsing, logging, and product policy |
| `repovec-systemd-probe` | Evidence types and ports for Podman generation and systemd verification | Unit parsing and live manager control |
| `repovec-secret-store` | Secret protection and durable repository composition | A specific encryption command or filesystem implementation |

_Table 1: Initial nursery crate boundaries._

The [`nursery crate guide`](nursery-crates.md) is the source of truth for
current interfaces, consumers, and graduation state. ADRs remain the source of
truth for accepted architectural decisions. The technical design describes how
production composition uses the decisions, and the roadmap owns delivery
status. Repeating full interface inventories across those documents is
deliberately avoided.

Production code may depend on a nursery crate only through an explicit roadmap
migration task. The dependency must not cross a public API boundary unless the
consumer also carries a clear instability notice. Nursery package names retain
the `repovec-` prefix so experimental interfaces cannot be mistaken for neutral,
community-supported crates.

A crate can graduate only after all of the following evidence exists:

- at least two independent production use cases exercise the abstraction;
- at least two interchangeable adapters or implementations prove that the
  interface does not merely mirror one backend;
- property, mutation, fuzz, or differential tests cover its critical
  invariants as appropriate;
- security, licence, minimum supported Rust version, and dependency reviews are
  complete;
- public documentation describes guarantees, non-guarantees, and failure
  semantics;
- an API review records naming, ownership, compatibility, and versioning;
- a migration plan identifies downstream consumers; and
- a maintainer explicitly approves publication and removes `publish = false`.

A nursery crate that fails to demonstrate reuse should be folded back into its
single consumer or deleted. Incubation is not a one-way conveyor belt.

## Goals and non-goals

Goals:

- make prospective abstractions concrete without premature publication;
- keep experimental dependencies out of the production workspace by default;
- create an evidence-based graduation process; and
- provide obvious deletion and consolidation paths.

Non-goals:

- promising backward compatibility for nursery APIs;
- creating a new crate for every helper;
- mirroring upstream parser or protocol crates; or
- treating repository colocation as proof of generality.

## Migration plan

1. Add the nested workspace, separate continuous integration workflow, and
   three interface-only crates.
2. Record every production adoption as a roadmap task linked to its governing
   ADR.
3. Add implementations only through parser, tool, credential, or filesystem
   adapters whose ownership is explicit.
4. Review the nursery at each minor appliance release and record whether each
   crate advances, consolidates, or is deleted.
5. Start publication work only after the graduation evidence is complete.

## Known risks and limitations

- A separate workspace can drift from the production toolchain. The workflow
  therefore uses the repository Rust setup and minimum supported version.
- Interface-only crates can become speculative architecture exhibits. Each must
  gain a production consumer or face deletion at the next review.
- Separate continuous integration adds another required maintenance surface,
  but keeps experimental dependencies from perturbing the main lockfile.

## Architectural rationale

The nursery acts as an architectural airlock. Code can acquire shape and test
pressure without escaping into the ecosystem before its hull has been checked.
