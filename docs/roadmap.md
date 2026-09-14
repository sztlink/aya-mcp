# Roadmap

## Gate 0: protocol draft, complete

- Score, Lease, CapabilityReport and Receipt contracts.
- Small reference validator using pinned schema and canonicalization libraries.
- Golden fixtures and receipt-chain tests.
- Honest threat model.

## Gate 1: contract review, in progress

- Review names and lifecycle against one concrete artistic score.
- Confirm Public and Worker surfaces have no accidental authority overlap.
- Freeze the first compatible `v0` fixture set only after the fake workcell exposes what is actually needed.

## Gate 2: Rust bootstrap, complete

- Rust 1.88.0 and official `rmcp` 3.3.0 pinned.
- Separate Public MCP and Worker MCP binaries compile over stdio.
- Rust schema validation reuses the golden fixtures and matches the Node digest.
- Native MCP 2026-07-28 discovery and legacy 2025-11-25 initialization are smoke-tested.
- Execution remains disabled and both servers report `contract_only`.

## Gate 3: synthetic workcell, next

1. Implement the minimum workcell lifecycle.
2. Build a deterministic fake DCC end-to-end.
3. Add Worker capability discovery, execution and capture against only that fake DCC.
4. Complete one `Score -> Candidate + Evidence -> Receipt -> expiration` flow.
5. Simulate crash, timeout, orphan process, source mutation, partial output, path escape, inconsistent receipt and a successful multi-iteration run.

This gate proves lifecycle semantics under process isolation. It does not prove operating-system confinement.

## Gate 4: contract hardening

- Port the semantic admission and receipt verifier needed by the synthetic workcell to Rust.
- Refine contracts only where end-to-end evidence proves a need.
- Keep promotion and any trusted receipt-digest anchor outside Worker authority.

## Gate 5: confinement proof

- Build and adversarially test one operating-system backend.
- Enforce read-only input, isolated writable output, process-tree reaping and network policy.
- Fail closed when required capabilities are unavailable.
- Do not use the word sandbox before this gate passes.

## Gate 6: Blender vertical slice

- Run one synthetic Blender transformation inside a real workcell.
- Permit autonomous Python.
- Prove source immutability and process-tree expiration.
- Deliver a derived `.blend`, renders and receipt.

## Later

- TouchDesigner adapter.
- After Effects adapter.
- FLAMA Space as a private or separately authorized reference implementation.
