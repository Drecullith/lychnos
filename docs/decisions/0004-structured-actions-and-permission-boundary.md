# ADR-0004: Structured Actions and Permission Boundary

Status: Accepted

## Context

Lychnos may eventually use AI providers to reason about events and suggest useful actions.

An AI provider must never have direct authority to execute arbitrary shell commands or mutate the system.

State-changing, privileged, destructive, or externally visible operations require explicit mediation by Lychnos.

Game Mode and Disabled mode must prevent actions from proceeding.

## Decision

Actions are represented as typed `ActionProposal` values inside the Lychnos core.

An action proposal includes:

- an opaque action identifier
- action kind
- required capability
- impact classification
- risk classification
- structured parameters
- human-readable reason
- proposer identity
- optional source event

The initial permission policy behaves as follows:

- Normal mode + read-only action: allowed
- Normal mode + state-changing action: user approval required
- Normal mode + external action: user approval required
- Normal mode + privileged action: user approval required
- Normal mode + destructive action: user approval required
- Game Mode: denied
- Disabled mode: denied

The permission layer evaluates structured actions rather than raw shell commands.

No real executor is implemented in this phase.

## Consequences

AI providers, future analyzers, and user interfaces can propose actions without gaining execution authority.

Permission policy remains independent from action generation.

Runtime safety state is enforced before execution can ever be introduced.

Future work may add capability grants, remembered approvals, scoped policies, richer risk rules, and a restricted executor without changing this fundamental boundary.
