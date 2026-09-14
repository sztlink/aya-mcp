# ADR 0001: Rust for supervision, not DCC reimplementation

Status: accepted for the v0 direction

## Context

AYA MCP needs process supervision, bounded authority, deterministic hashing, externally anchorable receipt digests and two small MCP processes. It also needs native access to Blender, TouchDesigner, After Effects and other creative applications.

A current ecosystem project, [`dcc-mcp-core`](https://github.com/dcc-mcp/dcc-mcp-core), already provides a Rust-powered DCC gateway with dynamic discovery, routing, skills, adapters, execution and observability. Rebuilding that layer would duplicate existing behavior and obscure AYA MCP's distinct contribution.

The official Rust MCP SDK, [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk), is a Tier 1 SDK. Version 3.3.0 implements MCP 2026-07-28, requires Rust 1.88 and is Apache-2.0 licensed.

Evidence consulted on 2026-09-14:

- `dcc-mcp-core` commit `bd09f6ed2fbc55538e597d803118fea0072c7a78`;
- `rmcp` release `3.3.0`, published 2026-09-10.

## Decision

Rust is the intended production material for:

- the workcell supervisor;
- process-tree lifetime and resource limits;
- lease admission and expiration;
- filesystem and network policy adapters;
- canonical hashing and receipt sealing;
- the Public MCP executable;
- the Worker MCP executable;
- a narrow client for an external DCC gateway.

Rust will not initially be used to:

- reimplement Blender Python;
- replace TouchDesigner Python;
- replace After Effects ExtendScript;
- create a universal DCC command vocabulary;
- fork or vendor `dcc-mcp-core`.

DCC-native languages remain at the application edge. The first integration will use a version-pinned process protocol rather than internal crate linkage.

## Bootstrap strategy

The v0 contract was bootstrapped with a small Node.js reference implementation and shared JSON fixtures. Rust 1.88.0 was then installed on the development host through rustup, and the initial workspace was added only after the contracts passed review. The future workspace will pin:

```text
Rust 1.88.0
rmcp 3.3.0, pinned exactly in the initial workspace
```

CI and the Node reference will share golden fixtures. No untested Rust source is included merely to signal intent.

## Consequences

Positive:

- clear separation between creative adapters and authority supervision;
- small distributable supervisor binaries;
- strong type and memory safety for security-sensitive lifecycle code;
- no reinvention of the existing DCC gateway ecosystem.

Costs:

- multiple languages remain necessary;
- Windows confinement still depends on correct Windows security APIs;
- integration compatibility must be pinned and tested;
- Node and Rust conformance implementations must not drift.
