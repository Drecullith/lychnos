# ADR 0045: Bounded Character Initiative

## Status

Accepted.

## Context

Lychnos should feel like a persistent companion rather than a passive command box.

That requires two distinct things:

- a stable character/persona that shapes how Lychnos speaks;
- bounded initiative so an intelligence provider may propose something useful without gaining permission to interrupt constantly or act autonomously.

The design must preserve Game Mode, Disabled mode, approval boundaries, and user control.

## Decision

The canonical persona now includes explicit conversation-style and initiative-style guidance in addition to traits and principles.

The core also owns a provider-neutral initiative model:

- `InitiativeMode`: Off, Quiet, Normal, Proactive;
- typed triggers and priorities;
- an inspectable `InitiativeCandidate` containing only a proposed message and brief reason summary;
- `InitiativeContext` describing runtime/user-interaction state;
- an `InitiativePolicy` gate with cooldown and interruption rules;
- a replaceable `InitiativeProvider` boundary for future real intelligence providers.

Initiative candidates contain no action grants, shell commands, executor handles, or mutation capability.

The policy currently guarantees:

- no proactive AI speech in Disabled mode;
- no proactive AI speech in Game Mode;
- Off mode suppresses all initiative;
- Quiet mode requires high-priority candidates;
- ordinary candidates do not interrupt active user interaction;
- noncritical candidates respect cooldown;
- critical candidates may bypass cooldown/user-interaction suppression in Normal runtime mode, but still gain no action authority.

The canonical Lychnos style explicitly prefers useful, concise, non-repetitive observations over chatter and forbids speaking merely to appear alive.

## Consequences

- A future AI provider can reason about context and propose proactive speech without bypassing runtime policy.
- Persona/character remains Lychnos-owned and model-independent.
- Proactive speech and autonomous execution remain separate capabilities.
- Game Mode and Disabled mode remain hard suppressors for AI initiative.
- A real intelligence provider, context collectors, persistence, and runtime scheduling are still required before initiative becomes live behavior.
