# Roadmap

## Gate 0: protocol draft, complete

- Score, Lease, CapabilityReport and Receipt contracts.
- Pinned reference validator, golden fixtures and Receipt-chain tests.
- Honest threat model.

## Gate 1: contract review and synthetic fixtures, complete

- Public and Worker authority surfaces are disjoint.
- The supported synthetic Score and fake-DCC behavior are pinned.
- The first compatible `v0` synthetic fixture set is frozen. Contract changes now require explicit compatibility review.

## Gate 2: Rust bootstrap, complete

- Rust 1.88.0 and official `rmcp` 3.3.0 pinned.
- Separate Public MCP and Worker MCP binaries over stdio.
- Rust validation matches the Node reference digest.
- MCP 2026-07-28 discovery and 2025-11-25 initialization are smoke-tested.

## Gate 3: synthetic workcell across MCP boundaries, complete

- Public MCP validates the pinned Score, creates workcells, reports status, requests cancellation and exposes external review material.
- Worker MCP exposes only `aya_capability_discover`, `aya_execute` and `aya_capture` against the pinned fake DCC.
- E2E proof crosses `Score -> Public MCP -> Worker MCP -> fake DCC -> Candidate + Evidence -> Receipt -> expiry`.
- Tested cancellation and Public shutdown paths produce terminal cancellation Receipts without candidate promotion in the deterministic E2E harness.
- Public independently checks source custody, output hashes, required evidence, lifecycle, Score and Lease binding, Receipt chain and Receipt digest.
- Review and promotion authority remain outside Worker MCP.
- The complete proof reports only `contract_only`. It is not a sandbox or OS security boundary.

## Gate 3.5: real Blender value proof

Before broadening infrastructure, test whether AYA adds enough value to justify its overhead:

1. Select one low-overhead Blender bridge without vendoring it.
2. Run one bounded autonomous task on synthetic, non-sensitive material.
3. Benchmark direct agent execution against the AYA path.
4. Measure setup time, wall time, correction iterations, evidence completeness, source custody, cleanup and operational complexity.
5. Publish one verdict: `CONTINUE`, `SIMPLIFY` or `STOP / REDESIGN`.

This gate requires Blender and bridge installation plus a safe execution environment. It must not run powerful code under a normal personal profile.

## Frozen backlog after Gate 3

No item below is part of the synthetic Gate 3 implementation:

### Contract hardening

- Broader semantic admission beyond the pinned synthetic Score.
- Compatible contract evolution and migration fixtures.
- Richer contextual Receipt verification for future DCC adapters.

### OS confinement

- Read-only source mounts and isolated writable output.
- Network policy, resource quotas and hostile process-tree escape tests.
- `os_enforced` reporting only after adversarial tests pass.

### Trust and provenance

- External trusted anchoring for Receipt digests.
- Signature format, key custody, transparency log or timestamp authority.
- Reproducible adapter and bridge provenance, SBOM and supply-chain policy.

### Real DCC adapters

- Blender vertical slice after Gate 3.5 verdict.
- TouchDesigner and After Effects only after the Blender pattern proves useful.
- FLAMA Space remains private or separately authorized.

### Operations

- Install and verify the Worker SIGTERM listener before Public readiness, eliminating the remaining immediate-cancellation registration race.
- Strengthen cancellation and shutdown guarantees under adversarial scheduling and Worker crashes.
- Durable registry and restart recovery.
- Retention, garbage collection and audit export.
- Administrative promotion channel outside Worker authority.
