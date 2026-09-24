# ADR-0005: Security Audit Model

Status: Accepted

## Context

Lychnos needs two different kinds of records:

- diagnostic logs for developers and troubleshooting
- security audit records describing security-relevant behavior

These must not be treated as the same thing.

The audit trail must answer questions such as:

- what happened
- when it happened
- what component initiated it
- which event or action it related to
- what decision or result occurred

Persistent audit storage is not yet implemented.

## Decision

The core defines structured audit records independently from ordinary diagnostic logging.

Audit records may represent:

- events observed
- actions proposed
- permission decisions
- runtime mode changes
- future execution attempts
- future execution results

The core also defines an `AuditSink` interface.

The initial machine-independent implementation is an append-only in-memory audit log used for tests and prototyping.

No record mutation or deletion API is exposed by the initial audit interface.

Persistent storage, cryptographic integrity, rotation, retention policy, serialization, and export format remain intentionally undecided.

## Consequences

Security-relevant activity has a dedicated domain model from the beginning.

Future storage implementations can replace the in-memory sink without changing the components that produce audit records.

Diagnostic logging can evolve independently and may use different retention and privacy rules.
