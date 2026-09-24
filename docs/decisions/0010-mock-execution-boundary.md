# ADR-0010: Mock Execution Boundary

Status: Accepted

## Context

Lychnos now has:

- structured action proposals
- live runtime safety state
- permission evaluation

The foundation phase still must not execute real system commands or perform privileged actions.

We need an end-to-end boundary that proves an action can travel through authorization without introducing actual execution.

## Decision

The core provides a `MockExecutor`.

The mock executor:

- accepts a structured `ActionProposal`
- evaluates it through the default permission policy
- consults the live `RuntimeController`
- never performs any real system operation

Possible outcomes are:

- `WouldExecute` for an action currently permitted without approval
- `AwaitingUserApproval` for an action requiring explicit approval
- `Blocked` when runtime safety policy denies the action

The mock executor performs permission evaluation itself rather than trusting a permission result supplied by its caller.

No shell commands, subprocesses, privileged helpers, filesystem mutations, network actions, or other real execution mechanisms are introduced.

## Consequences

Lychnos can test the complete proposal-to-authorization path safely.

Game Mode and Disabled mode remain part of the execution boundary.

Future real executors must preserve the same structured-action and live-safety guarantees and must re-check safety immediately before execution.
