# AYA workcell protocol v0

Status: draft

## Documents

| Schema | Purpose |
|---|---|
| `aya.score/v0` | Human origin, intent, invariants and permitted variation. |
| `aya.lease/v0` | Temporary authority requested for one workcell. |
| `aya.capability-report/v0` | Enforcement claims a runtime reports for admission. |
| `aya.receipt/v0` | Ordered process evidence, source verification and derived candidate. |

## Canonical paths

Protocol paths are relative POSIX paths:

- valid: `input/scene.blend`
- invalid: `/home/user/scene.blend`
- invalid: `C:/scene.blend`
- invalid: `../scene.blend`
- invalid: `input\\scene.blend`

A validator check is not a filesystem security boundary.

## Digests

The reference digest is SHA-256 over RFC 8785 JSON Canonicalization Scheme bytes encoded as UTF-8.

## Receipt chain

Each event includes:

- a sequential zero-based index;
- the previous event digest, or 64 zeroes for the first event;
- its own digest computed after removing `eventSha256`.

`receiptSha256` covers the complete receipt except its own field. The event chain and receipt digest establish internal consistency. They detect mutation only when the final receipt digest is anchored through a trusted external channel. Anyone able to rewrite the receipt can otherwise recompute every hash. This is not an identity signature.
