# ADR-0001: Initial Core Module Boundaries

Status: Accepted

## Context

Lychnos is a local-first ambient AI companion with a platform-independent Rust core.

The core must not depend directly on Omarchy, Linux-specific collectors, UI frameworks, voice systems, or a specific AI provider.

The first development phase must remain machine-independent and testable with mocks.

## Decision

The initial `lychnos-core` crate is divided into these conceptual modules:

- `event` — normalized Lychnos events
- `bus` — internal event/message transport
- `action` — structured proposed actions
- `permission` — authorization and approval decisions
- `audit` — security-relevant audit records
- `memory` — Lychnos-owned model-independent memory interface
- `config` — configuration model
- `runtime` — runtime safety state including Normal, Game Mode, and Disabled

These begin as modules inside one crate.

They will only become separate crates if future architectural pressure justifies the split.

## Consequences

The core remains small and cohesive while preserving clear internal boundaries.

Platform integrations and user interfaces may depend on the core.

The core must not depend on those integrations.

Implementation details such as the event bus technology, persistent memory backend, AI provider, and Omarchy integration remain intentionally undecided.
