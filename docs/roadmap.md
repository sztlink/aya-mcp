# Roadmap

## Gate 0: protocol draft

- Score, Lease, CapabilityReport and Receipt contracts.
- Small reference validator using pinned schema and canonicalization libraries.
- Golden fixtures and receipt-chain tests.
- Honest threat model.

## Gate 1: contract review

- Review names and lifecycle against one concrete artistic score.
- Confirm Public and Worker surfaces have no accidental authority overlap.
- Freeze the first compatible `v0` fixture set.

## Gate 2: Rust core

- Pin Rust and official `rmcp` SDK.
- Implement separate Public MCP and Worker MCP binaries.
- Reuse the same golden fixtures.
- Add a fake DCC gateway for end-to-end tests.

## Gate 3: confinement proof

- Build and adversarially test one operating-system backend.
- Fail closed when required capabilities are unavailable.
- Do not use the word sandbox before this gate passes.

## Gate 4: synthetic workcell

- Execute a full score against a fake creative application.
- Produce before and after evidence, candidate and sealed receipt.
- Keep promotion outside Worker MCP.

## Gate 5: Blender vertical slice

- Run one synthetic Blender transformation inside a real workcell.
- Permit autonomous Python.
- Prove source immutability and process-tree expiration.
- Deliver a derived `.blend`, renders and receipt.

## Later

- TouchDesigner adapter.
- After Effects adapter.
- FLAMA Space as a private or separately authorized reference implementation.
