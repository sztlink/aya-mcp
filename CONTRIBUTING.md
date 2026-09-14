# Contributing

AYA MCP is currently a protocol draft.

## Development

Requires Node.js 24 or newer.

```bash
npm test
```

## Principles

- Preserve the human origin of every score.
- Never overwrite an input artifact.
- Keep DCC-native grammar at the application edge.
- Do not add a generic DCC gateway. Integrate with existing gateways.
- Do not claim sandboxing without operating-system enforcement and escape tests.
- Public MCP and Worker MCP remain separate processes and authorities.
- Examples must be synthetic and free of client material.

## Contract changes

During `v0 draft`, breaking changes are allowed but must update schemas, fixtures, reference validation and documentation together.

Use normalized relative POSIX paths in protocol examples. Never commit hostnames, network shares, credentials or private infrastructure paths.
