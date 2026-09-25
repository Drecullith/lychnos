# ADR-0019: Mandatory Security Audit and Separate Diagnostics

Status: Accepted

Supersedes the audit-configurability portion of ADR-0007.

## Context

Lychnos has always distinguished two categories of operational records:

- security audit records for security-relevant behavior
- ordinary diagnostic records for development and troubleshooting

The initial configuration schema nevertheless exposed an `[audit].enabled` switch.

That conflicts with the security model. Runtime-mode transitions, permission decisions, execution attempts, and other security-relevant events must remain auditable regardless of whether ordinary troubleshooting output is enabled.

At the same time, ordinary diagnostics should remain independently configurable because they have different purposes, privacy considerations, verbosity, and retention requirements.

## Decision

Security auditing is a mandatory core safety mechanism.

The machine-independent configuration schema no longer exposes a security-audit enable/disable setting.

Attempts to supply an `[audit]` section are rejected as unknown configuration.

Ordinary diagnostics are modeled separately through:

- `DiagnosticLevel`
- `DiagnosticRecord`
- `DiagnosticSink`
- `InMemoryDiagnosticLog`

Diagnostics have their own configuration:

```toml
[diagnostics]
enabled = true
```

Diagnostic logging may be disabled without disabling or weakening the security audit trail.

Because this changes the accepted configuration shape, the configuration schema version advances from version 1 to version 2.

Version 1 configuration is rejected rather than silently reinterpreted.

The security audit and diagnostics systems intentionally remain separate abstractions:

```text
Security audit
  -> mandatory
  -> security-relevant
  -> AuditSink
  -> future integrity/retention guarantees

Diagnostics
  -> configurable
  -> troubleshooting/development
  -> DiagnosticSink
  -> independent verbosity/retention policy
```

## Consequences

Security-relevant records can no longer be disabled through ordinary Lychnos configuration.

Future persistent audit storage may rely on the invariant that security auditing is part of the runtime safety model.

Diagnostics can evolve independently with different verbosity, storage, rotation, retention, and privacy behavior.

Configuration files using schema version 1 must be migrated explicitly to schema version 2.

The foundation diagnostic sink is in-memory only and is not yet wired into the runtime orchestration path. This ADR establishes the domain and configuration boundary first.

Persistent security-audit storage, tamper resistance, diagnostics persistence, rotation, and retention remain future decisions.
