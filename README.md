# AYA MCP

**Autonomous workcells for creative software agents.**

AYA MCP explores a simple proposition:

> Give an agent broad creative capability inside a bounded workcell. Contain consequences at the environment boundary, not by making the agent incapable.

## Status

AYA MCP is an experimental `v0` protocol draft. This repository currently contains contracts, fixtures, a Node.js reference validator, two minimal Rust stdio MCP processes and a Linux-only synthetic workcell runner.

The synthetic runner supervises only the repository's deterministic fake DCC and reports `contract_only`. Public and Worker MCP are not yet wired into that lifecycle. The project does **not** yet provide:

- an operating-system sandbox;
- a Blender, TouchDesigner or After Effects integration;
- arbitrary code execution against real or untrusted DCCs;
- a production security boundary.

Do not describe the current code as sandboxing.

## The unit of work

AYA MCP does not treat a tool call as the primary unit. Its unit is a temporary **workcell**:

```text
human score
  -> bounded workcell
  -> autonomous inspect / execute / capture / correct loop
  -> derived candidate
  -> visual and technical evidence
  -> hash-linked receipt with an externally anchorable digest
  -> human review outside the worker
```

A score states the human origin, intent, invariants, permitted variation and evidence needed to look at the result. The worker may use raw Python, ExtendScript or native DCC operations when the environment actually contains their consequences.

## Relationship to DCC gateways

AYA MCP is not another universal DCC gateway.

[`dcc-mcp-core`](https://github.com/dcc-mcp/dcc-mcp-core) already provides a Rust-powered, gateway-first control plane with dynamic capability discovery, adapters, skills, routing and DCC execution. AYA MCP intends to use such gateways as external capabilities.

AYA MCP focuses on a different layer:

- the human score;
- workcell custody and expiration;
- truthful capability reporting;
- derived candidates instead of source overwrite;
- visual evidence;
- receipts and review.

## Contracts in `spec/v0`

- `Score`: origin, intent, invariants, variation space and evidence requirements.
- `Lease`: temporary authority requested for one workcell.
- `CapabilityReport`: claims a runtime makes about its effective enforcement.
- `Receipt`: ordered actions, source verification, typed evidence and candidate outputs.

All artifact paths use normalized relative POSIX paths. Absolute paths, backslashes and parent traversal are rejected.

## Reference validator

Requires Node.js 24 or newer. Dependencies are pinned in `package-lock.json`.

```bash
npm ci
npm test
node bin/aya-mcp.mjs validate spec/v0/fixtures/valid/score.json
node bin/aya-mcp.mjs digest spec/v0/fixtures/valid/score.json
node bin/aya-mcp.mjs verify-receipt \
  spec/v0/fixtures/valid/receipt.json \
  spec/v0/fixtures/valid/score.json \
  spec/v0/fixtures/valid/lease.json
```

This validator tests the protocol. It does not grant or enforce operating-system authority.

## Rust workspace

Requires the pinned Rust 1.88.0 toolchain:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run smoke:rust-mcp
cargo build --workspace --bins --locked
./target/debug/aya-synthetic-workcell /tmp/aya-workcell-demo success
cargo run -p aya-public-mcp
cargo run -p aya-worker-mcp
```

The MCP binaries use the official `rmcp` 3.3.0 SDK over stdio. The smoke test covers the native MCP 2026-07-28 `server/discover` path and the legacy 2025-11-25 `initialize` path. Status responses truthfully report `contract_only`, and the Worker reports execution disabled.

## Planned surfaces

The design keeps two distinct MCP processes:

- Public MCP: will accept scores, request workcells and expose status and review material.
- Worker MCP: will exist only inside a workcell and may discover and execute DCC capabilities.

The current Rust scaffolds already compile as separate binaries. Execution against real or untrusted DCCs remains disabled until OS confinement exists. The synthetic runner executes only a deterministic fake DCC as a dedicated child process with explicit deadline, staging, source verification and process-tree accounting. This is not a sandbox or security boundary. Public and Worker are not two modes of one process. Promotion of a candidate is outside the Worker MCP.

See [`docs/architecture.md`](docs/architecture.md) and [`docs/threat-model.md`](docs/threat-model.md).

## Gate 3 progress

The first synthetic slice now runs this loop without human intervention:

```text
Score -> admit -> fake DCC discover -> inspect -> capture
      -> weak execute -> diagnose -> correct -> execute
      -> save in attempt staging -> verify -> candidate + evidence
      -> receipt -> process-tree cleanup -> expiry
```

Tests cover successful multi-iteration work, crash and retry, timeout, global expiry, ordinary and detached orphan processes, source mutation including mutation followed by crash, partial output, unsafe returned paths, symlinked destinations and mounts, inconsistent evidence, a candidate that differs from the inspected scene, inconsistent receipts and multiple source inputs.

The fake process cannot be replaced with an arbitrary binary through the MCP surface. This slice proves lifecycle mechanics under `contract_only`; it does not prove operating-system confinement. Gate 3 remains incomplete until Public MCP creates the workcell and Worker MCP exposes fake-only discovery, execution and capture through the same lifecycle.

See [`docs/synthetic-workcell.md`](docs/synthetic-workcell.md) for the executable flow, scenarios and boundary.

## Origin

AYA MCP originates in Felipe Sztutman's desire for a digital technical worker that can receive an objective, work alone, inspect what it made, correct itself and deliver a versioned result.

## License

Apache-2.0. External DCC bridges keep their own licenses and are not vendored here.
