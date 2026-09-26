# ADR 0049: First Durable Local Memory Adapter

## Status

Accepted.

## Context

Lychnos already owns a provider-independent memory schema and MemoryStore boundary, but only an in-memory implementation existed.

Persistent local memory is central to companion continuity and later multi-body synchronization. The first durable adapter must not turn one database or file format into universal Lychnos canon.

The adapter also needs to preserve memory-schema validation, sensitivity/confidence metadata, provenance, device identity, and future synchronization revision/tombstone fields.

## Decision

The first durable memory adapter is implemented in the unprivileged desktop runtime as a versioned JSON snapshot store.

The core MemoryStore contract remains unchanged and backend-independent.

The Omarchy runtime adapter:

- loads from the user data directory under lychnos/memory-v1.json;
- treats a missing file as an empty store;
- does not create the file until the first actual memory write;
- validates the outer file schema and each MemoryRecord schema before accepting data;
- validates confidence percentages through the core constructor rather than trusting serialized integers;
- rejects duplicate memory IDs in one snapshot;
- preserves structured content, provenance, device ID, scope, sensitivity, confidence, sync revision, and tombstone metadata;
- rewrites records in deterministic ID order;
- writes through a temporary file, syncs it, and atomically renames it into place on the current Omarchy/Linux body;
- restricts the temporary memory file to owner read/write permissions on Unix;
- rolls the in-memory upsert back if persistence fails;
- fails closed on unsupported or malformed persisted data rather than silently overwriting it.

The runtime opens and validates the store during startup and reports the current record count/path.

This JSON snapshot is the first body-specific persistence adapter, not a final database decision. Android, iOS, Windows, macOS, Pocket Lychnos, or a future desktop implementation may use another backend behind the same MemoryStore contract.

Memory retrieval/context assembly is intentionally a separate next step. Merely having a durable store does not authorize every stored memory to be injected into every provider request.

## Consequences

- Lychnos now has a real durable local persistence boundary without coupling the core to SQLite or another storage engine.
- Memory can survive runtime restart once records are written.
- Corrupt or unsupported memory snapshots fail closed.
- Memory-provider privacy filtering and relevance retrieval remain explicit future policy rather than being hidden inside persistence.
- Multi-device synchronization can evolve using the existing revision/tombstone metadata without making this JSON file the synchronization protocol.
- A future storage backend can replace this adapter without changing Lychnos identity or memory semantics.
