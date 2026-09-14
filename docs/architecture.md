# Architecture v0

## Thesis

AYA MCP is a protocol for temporary creative workcells. It standardizes custody, evidence and lifecycle. It does not standardize the creative language of each DCC.

## Target flow

```text
Artist
  |
  | Score
  v
Public MCP process
  |
  | requests an isolated workcell
  v
OS confinement supervisor
  |
  +-- read-only input
  +-- private work directory
  +-- private writable candidate staging
  +-- bounded process tree and lifetime
  +-- explicit network policy
  |
  v
Worker MCP process
  |
  | dynamic discovery and execution
  v
External DCC gateway or native bridge
  |
  v
Candidate + evidence + receipt
  |
  v
Human review through an administrative channel outside Worker MCP
```

## Public and Worker are separate authorities

### Public MCP

The proposed public surface is fixed and does not expose DCC commands:

- `aya_score_validate`
- `aya_workcell_request`
- `aya_workcell_status`
- `aya_workcell_cancel`
- `aya_candidate_review`

### Worker MCP

The frozen Gate 3 Worker surface exists only inside a synthetic workcell:

- `aya_capability_discover`
- `aya_execute`
- `aya_capture`

It can select neither an arbitrary executable nor arbitrary code. All three tools target the pinned fake DCC. This is proof of separate process orchestration, not proof of process isolation. Future real-DCC authority requires a separately reviewed surface and an OS-enforced boundary.

`aya_execute` will be powerful in a real adapter. Its future safety depends on the workcell boundary, not on pretending arbitrary code can be validated semantically.

The Public MCP and Worker MCP must be separate executables, processes and policies. A configuration flag is not sufficient separation.

## Control plane boundary

AYA MCP does not reimplement DCC capability discovery, main-thread dispatch or application adapters. The Worker MCP will call an external gateway such as `dcc-mcp-core` or an application-specific bridge.

The initial integration preference is a process boundary with a pinned protocol and version. This avoids coupling AYA MCP to another project's internal Rust crates.

## Lifecycle

The synthetic fake-DCC proof comes before any real DCC or confinement claim. Gate 3 crosses the separate Public and Worker MCP processes under `contract_only`; the Public validates the returned lifecycle, custody, evidence and Receipt before exposing review material. A workcell follows this state model:

```text
requested -> admitted -> running -> candidate_ready -> sealed -> expired
                 |          |              |
                 v          v              v
               denied     failed        cancelled
```

Key invariants:

1. A lease describes authority but does not create it.
2. Admission fails when required confinement exceeds reported capability.
3. Inputs are immutable from the worker's perspective.
4. Outputs are candidates derived from input digests.
5. The worker cannot see or write the promotion destination.
6. Expiration reaps the complete process tree.
7. Receipts describe process and provenance, not artistic quality.

## Visual correction loop

Inside a running workcell the agent may:

```text
inspect -> discover -> execute -> capture -> diagnose -> correct
```

The loop is bounded by the score budget. Repeated equivalent failures require a changed approach or termination. Evidence records both successful and unresolved transformations.

## Language strategy

- Rust: contracts, separate MCP processes and the synthetic lifecycle supervisor; real confinement remains backlog.
- Python, TypeScript and ExtendScript: native DCC edges.
- Node.js: executable cross-language reference for the draft contracts and stdio smoke harness.

See [`adr/0001-rust-supervisor-not-dcc-gateway.md`](adr/0001-rust-supervisor-not-dcc-gateway.md).
