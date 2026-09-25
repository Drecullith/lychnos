# ADR-0020: Bound Action Approval Grants

Status: Accepted

## Context

Phase 1 deliberately stopped state-changing, external, privileged, and destructive actions at:

```text
PermissionDecision::RequiresUserApproval
```

That was safe, but it left a critical future boundary undefined: what exactly does a user's approval authorize?

Binding approval only to an action ID would be insufficient. A proposal could retain the same identifier while its parameters, action kind, capability, impact, risk, reason, proposer, or source-event linkage changed after the user reviewed it.

Lychnos also requires runtime safety to remain authoritative after approval. Approval granted while Lychnos is in Normal mode must not override a later transition into Game Mode or Disabled.

## Decision

Lychnos introduces an `ApprovalGrant` representing explicit approval for one exact structured `ActionProposal`.

The grant stores a snapshot of the complete proposal.

Matching uses structural equality against the complete proposal rather than action ID alone.

Therefore:

```text
approved proposal A != modified proposal A'
```

even when both proposals carry the same `ActionId`.

The `ApprovalGrant` constructor is crate-private. External components cannot directly mint a grant through the public type API.

The current trusted issuance boundary is:

```text
FoundationRuntime::approve_action(...)
```

Issuance appends an `ActionApproved` security-audit record containing:

- action ID
- source event when present
- approving actor label
- action kind
- required capability
- action impact
- action risk

The grant intentionally does not implement `Clone`.

The mock executor accepts an approval grant by value. This models one approval attempt and prevents casual reuse of the same grant object after it has been submitted.

Before considering approval, the executor evaluates the live `RuntimeController`.

Runtime safety therefore remains stronger than approval:

```text
valid approval + Normal   -> may proceed through mock execution
valid approval + GameMode -> blocked
valid approval + Disabled -> blocked
```

A mismatched grant does not authorize execution and returns the action to the approval-required state.

The security audit records whether the execution evaluation saw approval as:

- `not_required`
- `missing`
- `matched`
- `mismatched`
- `runtime_blocked`

No real system execution is introduced by this decision.

## Consequences

Approval is bound to what the user reviewed rather than merely to an identifier.

Changing parameters or security metadata invalidates an existing grant.

A runtime-mode change after approval still blocks the action.

The approval issuance event and later permission evaluation remain separately auditable.

The design creates a safe seam for a future UI, voice interaction layer, or other trusted consent mechanism.

## Current Limitations

`FoundationRuntime::approve_action(...)` is a foundation trust boundary, not an authenticated human-identity system.

The `approved_by` value is currently a caller-supplied actor label. It does not prove OS identity, biometric identity, device identity, or presence of a human user.

There is no expiration timestamp, persistent approval store, cryptographic signature, nonce, or cross-process replay protection yet.

An authorized caller can request more than one new grant by invoking the approval boundary more than once.

These limitations are acceptable while the executor remains mock-only, but they must be addressed as appropriate before real action execution is introduced.
