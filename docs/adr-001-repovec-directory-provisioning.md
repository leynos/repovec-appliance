# Architectural decision record (ADR) 001: Provision the `repovec` system user and private directory tree

## Status

Accepted.

## Date

2026-08-24.

## Context and problem statement

repovec-appliance is a single-owner appliance that turns private GitHub
repositories into a continuously indexed, MCP-queryable corpus. The service
layout (roadmap item 1.3.1) and the per-repository indexer template (1.3.2) run
their daemons as the `repovec` system user with `HOME=/var/lib/repovec`, but
nothing creates that account's home data tree or the account itself on a
freshly provisioned host. `systemd-sysusers` creates the passwd/group record
but never the home directory; a systemd `StateDirectory=` requires root to own
the tree or a root provisioning hand-off; and a boot-time `tmpfiles` pass has
not run on an un-rebooted host. The appliance therefore needs a deterministic
provisioning mechanism whose contract is validated before any daemon starts,
both statically in CI and live at daemon startup.

## Decision Drivers

- Private repository confidentiality: mirrors and derived indexes must not be
  world- or group-readable.
- Deterministic first-boot provisioning: `systemctl start repovec.target` must
  succeed on an un-rebooted, freshly installed host.
- Declarative, idempotent assets that match the appliance's checked-in
  packaging style and can be statically validated from Rust.
- Single provisioning authority for `/etc/repovec` secrets.
- Rootless Qdrant compatibility as an explicit future dependency.

## Requirements

### Functional requirements

- Create the `repovec` system user with home `/var/lib/repovec` and shell
  `/usr/sbin/nologin`.
- Create `/var/lib/repovec` and the `git-mirrors/`, `worktrees/`, `.grepai/`,
  and `qdrant-storage/` children before any dependent service starts.
- Provision without requiring a reboot.
- Fail closed at daemon startup when the live tree is misprovisioned.

### Technical requirements

- Provisioning assets must be checked-in files under `packaging/` that a pure,
  I/O-free validator can parse and compare against a per-directory contract.
- `RuntimePaths` stays a pure path type; the mode/owner/group policy lives in
  the validator's spec table.
- No numeric uid/gid may be pinned; ownership checks resolve by name.

## Options considered

### Option A: `tmpfiles.d` + `sysusers.d` applied by a target-start oneshot

Ship `packaging/sysusers.d/repovec.conf` (existing) and
`packaging/tmpfiles.d/repovec.conf` (new), and run both from a
`repovec-provision.service` oneshot that executes `systemd-sysusers` before
`systemd-tmpfiles --create`. The oneshot is `WantedBy=repovec.target` and
ordered `Before=` the API-key oneshot, Qdrant, and both daemons. Both tools are
declarative, idempotent, and do not empty populated directories.

### Option B: unit `StateDirectory=` / `RuntimeDirectory=`

Declare `StateDirectory=repovec` in the daemon units and let systemd create the
tree. This requires root-owned provisioning (or a `User=` hand-off), does not
cover the `repovec` account creation, and cannot express the heterogeneous
per-directory ownership (`root:root` `qdrant-storage` versus `repovec:repovec`
data dirs) in one contract.

### Option C: imperative helper (libexec script) provisioning the tree

Extend the existing `repovec-qdrant-api-key` helper to also create the data
tree. This concentrates more filesystem policy in a shell helper that is not
statically validated from Rust and duplicates the appliance's declarative asset
pipeline.

| Topic                             | Option A        | Option B                      | Option C               |
| --------------------------------- | --------------- | ----------------------------- | ---------------------- |
| Deterministic on un-rebooted host | Yes             | No (`/var/lib` boot tmpfiles) | Yes                    |
| Declarative, statically validated | Yes             | Partly (unit text)            | No                     |
| Heterogeneous per-dir owner/mode  | Yes             | No                            | Yes                    |
| Idempotent and convergent         | Yes             | Yes                           | Partly                 |
| Single secrets authority          | Yes (unchanged) | Yes                           | Violates (two writers) |

_Table 1: Comparison of provisioning options._

## Decision outcome / proposed direction

Adopt Option A. The appliance ships `packaging/tmpfiles.d/repovec.conf` beside
the existing `packaging/sysusers.d/repovec.conf` and applies the exact
sysusers-before-tmpfiles pair from `repovec-provision.service` at target start.

The checked-in `tmpfiles.d` asset declares the data tree with explicit modes
and owners (`d` entries, never the world-readable `0755` default):

```text
d /var/lib/repovec 0700 repovec repovec -
d /var/lib/repovec/git-mirrors 0700 repovec repovec -
d /var/lib/repovec/worktrees 0700 repovec repovec -
d /var/lib/repovec/.grepai 0700 repovec repovec -
d /var/lib/repovec/qdrant-storage 0700 root root -
```

The sysusers asset keeps the existing contract
(`u repovec - "repovec appliance service user" /var/lib/repovec /usr/sbin/nologin`).
`/etc/repovec` stays `0750 root:repovec`, provisioned exclusively by the
libexec helper (SI-4); the `tmpfiles.d` asset must never declare it, and the
validator rejects it if it does.

`qdrant-storage` is `root:root` `0700` because Qdrant runs as uid 0 under
rootful Podman and is the only filesystem accessor; root traverses the
`0700 repovec` parent via the DAC override. This ownership is valid only while
Qdrant remains rootful with no userns remap: if `UserNS=auto`, rootless Qdrant,
or a `User=` directive is ever added, the contract must change.

The runtime backstop is `repovec-core::appliance::directory_layout::live`,
which stats the real tree at daemon startup and refuses to start on any owner,
group, mode, or missing-directory violation, logging the offending path. The
pure validator (`directory_layout`) and the live pre-flight together enforce
the appliance directory contract in CI and on the host.

## Goals and non-goals

- Goals:
  - Provision the `repovec` account and private data tree deterministically at
    target start, without a reboot.
  - Validate the packaging contract statically (CI gate) and live (daemon
    pre-flight).
  - Keep `/etc/repovec` single-authority.
- Non-goals:
  - Managing repository mirrors or worktrees (roadmap items 2.x).
  - Provisioning Qdrant itself (1.2.x owns the Quadlet and API-key service).
  - Enforcing live-layout checks inside the pure validator; the live adapter
    is a separate, explicitly I/O-bound module.

## Known Risks and Limitations

- `systemd-tmpfiles` `d` lines do not repair misowned _existing children_; the
  live pre-flight detects that case rather than silently tolerating it.
- The `qdrant-storage` `root:root` ownership assumes Qdrant remains rootful
  with no userns remap; any change to the Quadlet's user/namespace handling
  invalidates the directory contract and must be coordinated with this ADR.
- `repovec-provision.service` assumes the host provides `systemd-sysusers`,
  `systemd-tmpfiles`, and `systemd-sysusers`-time account records; the static
  validator cannot prove the host has those binaries.

## Architectural Rationale

The appliance already treats packaging assets as declarative, checked-in inputs
that `repovec-core` validates from Rust. Option A extends that pattern instead
of introducing a second imperative provisioning path, preserves the
single-authority `/etc/repovec` contract, and leaves a live fail-closed
backstop so static text correctness is never mistaken for host correctness.
