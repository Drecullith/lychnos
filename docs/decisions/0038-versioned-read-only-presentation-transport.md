# ADR-0038: Versioned Read-Only Presentation Snapshot Transport

Status: Accepted

## Context

The machine-independent core already exposes `CompanionPresentationState` as an owned, authority-free projection.

The Omarchy floating shell previously constructed demo presentation state locally from `LYCHNOS_DEMO_STATE`. That proved rendering, but it did not prove that the visible companion could follow a separately owned `FoundationRuntime`.

The presentation process must not gain an executor, approval grant, runtime controller, or mutable runtime handle merely to display changing state.

## Decision

Make the read-only presentation projection serializable and transport it inside a versioned `CompanionPresentationEnvelope`.

The current schema version is `1`.

The foundation CLI can publish snapshots atomically to a session-local path:

`$XDG_RUNTIME_DIR/lychnos/presentation-v1.json`

An explicit `LYCHNOS_PRESENTATION_PATH` may override the path for development/testing.

The isolated GTK shell reads and validates that versioned envelope, keeps only the latest `CompanionPresentationState`, and redraws when the snapshot changes.

The prototype currently checks the small local snapshot file on a short timer. This is a Phase 2 transport prototype, not a permanent IPC decision.

## Security Consequences

Serialization is limited to presentation-domain data.

The wire representation contains no:

- `ApprovalGrant`;
- executor;
- `RuntimeController`;
- mutable `FoundationRuntime` reference;
- permission bypass; or
- direct host-action method.

Unknown presentation schema versions fail closed during decoding.

The shell can display that approval is required, but receiving the snapshot does not grant approval authority.

## Consequences

The desktop body can now react to real `FoundationRuntime` state while remaining a separate presentation process.

Environment-variable demo state is no longer required for the live shell path.

A future IPC mechanism may replace the JSON snapshot transport without changing the core read-only presentation contract.
