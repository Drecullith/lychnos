# ADR-0028: Pending Action Lifecycle Scenarios

Status: Accepted

## Context

Phase 2 already had a deterministic multi-step scenario runner capable of combining collector polling, runtime-mode transitions, collector failures, and recovery against one continuous `FoundationRuntime`.

Pending actions were still exercised mainly through isolated orchestrator tests.

Now that approval, rejection, and system-driven cancellation all have explicit runtime semantics, the simulator needs to compose those lifecycle operations into end-to-end deterministic stories.

## Decision

Extend the machine-independent scenario layer with pending-action lifecycle steps.

`ScenarioStep` now supports:

- evaluating one structured `ActionProposal`;
- approving one exact pending proposal with an actor label;
- explicitly rejecting one exact pending proposal with an actor label;
- cancelling one exact pending proposal for a structured system-owned reason;
- the existing collection and runtime-mode transition operations.

The proposal values carried by lifecycle steps are boxed so the scenario-step enum does not grow unnecessarily large.

## Result Model

`ScenarioStepResult` now represents:

- action evaluation outcomes;
- approval results;
- rejection results;
- cancellation results;
- existing collection results;
- existing runtime transitions.

Lifecycle errors are preserved as the existing typed `PendingActionError` values.

The scenario runner does not reinterpret or bypass runtime semantics.

## Runtime Authority

All lifecycle steps delegate directly to the existing `FoundationRuntime` APIs:

```text
EvaluateProposal
    -> evaluate_proposal(...)

Approve
    -> approve_pending_action(...)

Reject
    -> reject_pending_action(...)

Cancel
    -> cancel_pending_action(...)
```

Therefore the scenario layer does not gain independent approval, rejection, or cancellation authority.

It only composes already-defined runtime boundaries.

## Tested Lifecycle Stories

The scenario layer now proves the following complete flows.

### Approval

```text
evaluate state-changing proposal
-> awaiting approval
-> approve exact proposal
-> mock WouldExecute
-> pending entry removed
-> ActionApproved present in security audit
```

### Explicit Rejection

```text
evaluate state-changing proposal
-> awaiting approval
-> reject exact proposal
-> pending entry removed
-> ActionRejected records the supplied user actor label
```

### System Cancellation

```text
evaluate state-changing proposal
-> awaiting approval
-> cancel exact proposal for PolicyInvalidated
-> pending entry removed
-> ActionCancelled records policy_invalidated
```

### Runtime-Safety Retry

```text
evaluate state-changing proposal
-> awaiting approval
-> Disabled
-> exact approval attempted
-> Blocked
-> proposal remains pending
-> explicit return to Normal
-> same exact proposal approved
-> mock WouldExecute
-> pending entry removed
```

This confirms that scenario composition preserves the accepted retry semantics around Disabled mode.

## Security Consequences

The simulator can now exercise full pending-action lifecycle behavior without inventing new authority.

The following distinctions remain intact:

- approval is explicit positive consent;
- rejection is explicit negative consent;
- cancellation is system-owned lifecycle invalidation;
- Disabled can block approval without deleting pending state;
- later re-enable can allow retry of the same exact pending proposal.

All security-relevant events remain produced by the runtime and written through the existing mandatory security-audit boundary.

## Current Limitations

The scenario layer still does not model:

- authenticated user identity;
- approval expiry;
- persistent pending state;
- restart recovery;
- concurrent approvals;
- cancellation of already-running work;
- real executor behavior;
- real operating-system actions;
- multiple runtime instances or devices.

Those remain separate boundaries.
