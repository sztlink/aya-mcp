# AYA MCP

**Autonomous workcells for creative software agents.**

AYA MCP explores a simple proposition:

> Give an agent broad creative capability inside a bounded workcell. Contain consequences at the environment boundary, not by making the agent incapable.

## Status

AYA MCP is an experimental `v0` protocol draft. This repository currently contains contracts, fixtures and a small Node.js reference validator.

It does **not** yet provide:

- an MCP server;
- an operating-system sandbox;
- a Blender, TouchDesigner or After Effects integration;
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

## Planned surfaces

The design keeps two distinct MCP processes:

- Public MCP: accepts scores, requests workcells and exposes status and review material.
- Worker MCP: exists only inside a workcell and may discover and execute DCC capabilities.

They are not two modes of one process. Promotion of a candidate is intentionally outside the Worker MCP.

See [`docs/architecture.md`](docs/architecture.md) and [`docs/threat-model.md`](docs/threat-model.md).

## First proof

The first vertical proof will use one Blender workcell and one synthetic scene:

1. mount an input `.blend` read-only;
2. run Blender and its bridge inside the workcell;
3. permit autonomous Python execution;
4. capture before and after evidence;
5. save a derived `.blend` candidate;
6. record independently computed source digests before and after execution;
7. seal a self-consistent receipt and anchor its digest outside the worker;
8. expire the workcell and reap its process tree.

No claim of confinement will be made until escape tests pass against a real operating-system boundary.

## Origin

AYA MCP originates in Felipe Sztutman's desire for a digital technical worker that can receive an objective, work alone, inspect what it made, correct itself and deliver a versioned result.

## License

Apache-2.0. External DCC bridges keep their own licenses and are not vendored here.
