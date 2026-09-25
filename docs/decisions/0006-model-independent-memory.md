# ADR-0006: Model-Independent Lychnos Memory

Status: Accepted

## Context

Lychnos memory must belong to Lychnos rather than to ChatGPT, a local LLM, or any other AI provider.

Memory must remain usable if:

- the AI provider changes
- Lychnos operates offline
- the desktop implementation changes
- a portable Lychnos device is added later
- memory synchronization is introduced later

The permanent persistence backend and synchronization protocol are not yet decided.

## Decision

The Lychnos core defines its own memory record schema and storage interface.

A memory record includes:

- an opaque memory identifier
- schema version
- memory kind
- structured content
- creation and update timestamps
- provenance
- optional originating device
- scope
- sensitivity classification
- optional confidence
- synchronization metadata
- tombstone metadata for future deletion/synchronization semantics

The core defines a `MemoryStore` interface.

The initial machine-independent implementation is an in-memory store used for tests and prototyping.

The interface returns owned memory records so future implementations may use SQLite, another local database, or another persistence mechanism without exposing implementation-specific references.

No hard-delete API is introduced during this phase.

SQLite remains a likely future local backend but is not selected by this ADR.

## Consequences

AI providers consume memory supplied by Lychnos rather than owning the authoritative memory state.

Desktop and future portable implementations can share a common memory domain model.

Persistence, encryption at rest, search/indexing, synchronization transport, conflict resolution, retention policy, and permanent deletion semantics remain intentionally undecided.
