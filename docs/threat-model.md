# Threat model v0

## Current guarantee

The current repository validates protocol documents, source and evidence relationships, event hash chains and the complete receipt digest. It provides no operating-system confinement or trusted external receipt anchor.

## Protected assets

A production workcell must protect:

- source project integrity;
- approved deliveries;
- credentials and user profiles;
- files outside the assigned task;
- network destinations outside policy;
- host availability;
- provenance of candidates and evidence.

## Adversary model

Assume worker-authored Python, ExtendScript and DCC scripts may be incorrect or hostile. They may attempt to:

- traverse paths or follow symlinks outside the cell;
- mutate the source;
- start child processes;
- survive cancellation;
- access credentials or unrelated files;
- open outbound network connections;
- exhaust CPU, memory, disk or render time;
- forge or reorder receipt events;
- overwrite an approved artifact.

## Levels reported by a runtime

- `contract_only`: conventions and application validation only.
- `process_isolated`: process lifecycle is bounded, but filesystem or network isolation is incomplete.
- `os_enforced`: filesystem, process and network boundaries are enforced and tested by the operating system.

A runtime must fail closed when a score requires a stronger level than it can provide.

## Non-guarantees

- A lease is not a credential and does not create sandboxing.
- A Job Object controls lifecycle and resources but is not a complete Windows security boundary.
- Relative-path checks do not stop a process with ordinary user permissions from opening other paths.
- Hash chains and a receipt digest provide internal consistency only. Without a trusted external anchor, an attacker can rewrite the receipt and recompute every hash.
- A screenshot proves captured pixels, not artistic quality.
- Separating MCP processes does not by itself separate operating-system authority.

## Production boundary requirements

Before advertising `os_enforced`, a backend must demonstrate:

1. source mounted read-only;
2. output limited to a fresh candidate directory;
3. no access to the user's profile, secrets or unrelated drives;
4. explicit outbound network policy;
5. complete child-process reaping on timeout or cancellation;
6. resource and output limits;
7. resistance to symlink and path traversal escapes;
8. promotion destination invisible to the worker;
9. adversarial tests with recorded results.

## Reporting vulnerabilities

Do not open a public issue for a vulnerability that could expose user data or escape a workcell. Follow [`SECURITY.md`](../SECURITY.md).
