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

## Gate 3.5: real Blender value proof, complete

- CallMeJones Blender Agent Bridge 0.5.6 was used unchanged with portable Blender 5.1.2 on the 4090 Render Server.
- A direct arm and a minimal AYA Worker arm used the same model, thinking, briefing, source, bridge and budget.
- Both arms completed autonomously with source custody and technically valid candidates.
- The AYA arm was faster and used fewer bridge calls and total tokens including cache, but had two self-corrected integration failures and scored lower in blind visual review by an isolated LLM evaluator, not a human evaluator.
- Human intervention was zero in both arms, so the proposed supervision advantage was not demonstrated.
- Verdict: `SIMPLIFY`. Preserve only a thin custody, budget and restricted-tool layer until a randomized replication demonstrates non-inferior quality.

See [`experiments/gate-3.5/README.md`](../experiments/gate-3.5/README.md).

### Gate 3.5R: replication only, complete

- Three A and three B runs used a randomized route order and a byte-identical neutral prompt.
- All six completed autonomously, preserved the source, produced candidates and were judged usable for AYA continuity.
- B was faster in all three observed runs and the sample ranges were disjoint: A 305.876 to 316.676 s and B 291.358 to 301.428 s.
- B wall time was about 2.4% lower by median and 4.0% lower by mean; the exact two-sided permutation p-value was 0.10.
- Do not attribute this separation causally to AYA: B used more tokens, bridge calls and agent tool calls, so the observed mechanism does not explain the wall-time advantage.
- The earlier visual deficit did not repeat; visual medians from an isolated LLM evaluator, not a human evaluator, were A 92 and B 93.
- Human intervention remained zero for both paths.
- The replication sustains `SIMPLIFY`: retain only the thin custody, budget and tool-restriction hypothesis.

See [`experiments/gate-3.5R/README.md`](../experiments/gate-3.5R/README.md). No Gate 4, sandbox, TouchDesigner or After Effects work follows automatically from these results.

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
