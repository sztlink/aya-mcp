# Synthetic workcell

The Gate 3 laboratory is a Linux-only `contract_only` lifecycle proof. It is not an operating-system sandbox and does not accept an arbitrary DCC executable through MCP.

## Run

```bash
cargo build --workspace --bins --locked
./target/debug/aya-synthetic-workcell /tmp/aya-workcell-demo success
npm run e2e:synthetic-mcp
```

The first command runs the library-facing laboratory executable. The E2E command crosses the separate Public MCP and Worker MCP processes for success, cancellation and Public shutdown.

The destination must not already exist. A successful run leaves the synthetic source under `input/` and reviewable candidate material under `output/`.

## Autonomous loop

```text
request -> admit -> running
  -> discover fake capabilities
  -> inspect source
  -> capture before
  -> execute weak variation
  -> inspect and diagnose
  -> correct and execute again
  -> inspect final state
  -> capture after
  -> save into private attempt staging
  -> terminate and account for the fake process tree
  -> rehash every source
  -> validate and promote the candidate
  -> write and reread the receipt
  -> sealed -> expired
```

The fake DCC speaks a private NDJSON protocol over stdin and stdout. It exists only to exercise workcell behavior and is not proposed as a public DCC standard.

## Scenarios

| Scenario | Expected behavior |
| --- | --- |
| `success` | Two creative iterations, candidate, evidence and valid receipt |
| `crash-once` | First process crashes; typed transient failure retries within budget |
| `timeout` | Bounded request timeout; no candidate is promoted |
| `orphan` | Child left in the DCC process group is terminated and reaped |
| `detached-orphan` | Reported child in another process group is terminated and reaped |
| `source-mutation` | Custody violation is detected and never retried |
| `mutation-then-crash` | Mutation is detected after crash and blocks retry |
| `partial-output` | Attempt staging is removed; partial output is not promoted |
| `path-escape` | Unsafe returned artifact path is rejected lexically |
| `inconsistent-evidence` | DCC-reported digest is checked against the actual file |
| `candidate-mismatch` | Saved candidate is compared with the final inspected scene |
| `inconsistent-receipt` | Event-chain corruption is rejected before publication |

Additional tests cover multiple Score inputs, global lease and Score deadline, symlinked output targets, symlinked mounts and stronger isolation requests.

## Honest boundary

The runner controls the pinned fake process lifecycle and verifies what it produces. It does not prevent a hostile process from accessing the host, network or unrelated files. Path and custody checks detect specific violations; they do not create confinement.

Only a later OS backend may report `os_enforced`. Gate 3 connects the separate Public MCP and Worker MCP processes to this lifecycle, while remaining explicitly `contract_only`. Review material is exposed by Public; Worker has no review or promotion authority.
