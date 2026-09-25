# ADR 0039: Presentation Control Intents

## Status

Accepted.

## Context

The Phase 2 visual shell already consumes a read-only versioned presentation snapshot from a separately owned `FoundationRuntime`.

Pending actions must eventually be approved or rejected by the user, but giving the shell an executor, approval grant, runtime controller, or mutable runtime reference would collapse the intended security boundary.

A decision emitted by the UI must also be protected against a stale shell approving a newer proposal that reused the same action ID.

## Decision

Lychnos uses a separate versioned presentation-to-runtime control contract.

For pending approvals, the shell emits only:

- the pending action ID;
- an opaque proposal binding;
- an `Approve` or `Reject` decision.

The proposal binding is a SHA-256 digest of the exact in-memory proposal representation projected by the runtime.

The shell writes each control request atomically as a separate JSON file in the session-local control inbox:

`$XDG_RUNTIME_DIR/lychnos/control-inbox-v1/`

The runtime owner:

1. parses and schema-validates the request;
2. looks up the currently pending full `ActionProposal`;
3. recomputes and compares the proposal binding;
4. rejects stale, unknown, or mismatched requests;
5. invokes the existing exact `approve_pending_action` or `reject_pending_action` boundary.

The shell never receives the approval grant and never calls an executor directly.

The binding is a Phase 2 stale-decision/freshness guard. It is not user authentication, IPC authentication, or a persistent cross-version identity.

## Consequences

- Approve/Reject controls can exist in the shell without granting it execution authority.
- Same-ID proposal replacement cannot silently inherit a stale UI decision when the proposal representation changes.
- Multiple control requests can be written without overwriting a single mailbox file.
- The transport remains replaceable; a future authenticated IPC channel may supersede the session-file inbox.
- Future typed interaction requests can reuse the same authority-separation pattern.
- Persistent request delivery, authenticated user identity, replay protection across sessions, and production IPC remain future work.
