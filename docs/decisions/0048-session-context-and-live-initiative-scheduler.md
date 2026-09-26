# ADR 0048: Session Context and Context-Triggered Live Initiative

## Status

Accepted.

## Context

A real local model can generate natural single-turn replies, but a companion also needs short-term conversational continuity and a safe way to decide when something is worth saying without being prompted.

Persistent synchronized memory remains a separate Lychnos-owned subsystem and is not yet implemented. The initiative system must therefore avoid pretending that transient model context is durable memory.

Proactive behavior must also avoid repeated model polling, chatter for its own sake, interruptions, Game Mode activity, and any path from a thought into host-action authority.

A key design distinction is required:

- a meaningful context change may justify considering initiative;
- user idleness only answers whether it is an appropriate time to interrupt.

Silence by itself is not a reason for Lychnos to invent something to say.

## Decision

The first live LocalBrain adapter keeps a bounded session-local conversation window:

- six user/assistant turns (twelve messages maximum);
- held only in runtime memory;
- supplied to later conversation turns and initiative proposals;
- cleared when the runtime restarts;
- separate from persistent Lychnos memory.

The runtime owns a conservative, context-triggered initiative scheduler.

InitiativeContext carries an explicit InitiativeTrigger so the intelligence provider knows why initiative is being considered. Current and future trigger categories include conversation follow-up, diagnostic/context change, memory cue, pending approval, scheduled check, and user-idle metadata.

The scheduler keeps user activity and meaningful context changes as separate signals:

- user activity updates the interruption guard only;
- a successfully handled conversation currently creates a ConversationFollowUp context trigger;
- future collectors and memory retrieval may create their own explicit triggers;
- runtime-mode changes do not automatically create a new thought opportunity merely because a mode changed.

A candidate may be considered only when:

- at least one real session context exists;
- an unconsidered meaningful trigger exists;
- the user has been quiet for at least 60 seconds;
- the runtime is in Normal mode;
- no approval is currently pending.

For unchanged context, the scheduler asks the intelligence provider at most once. It does not repeatedly poll the model searching for something to say.

The intelligence provider receives the exact trigger and may return no candidate. If it returns one, the existing core InitiativePolicy remains authoritative and may still suppress it.

A surfaced initiative:

- receives an initiative-prefixed interaction ID;
- is published through the normal conversation-response transport;
- may be spoken through the normal TTS worker;
- is recorded in session context only after it was actually surfaced;
- carries no executor, approval grant, command, or mutation authority.

The shell displays proactive responses in the existing compact chat panel without taking keyboard focus. A proactive response waits when the shell is already waiting for a direct user reply.

## Consequences

- Lychnos can maintain natural short-term conversational continuity without confusing it with durable memory.
- Silence alone never opens a new initiative window.
- User idleness is an interruption guard, not a content-generation trigger.
- The same trigger vocabulary can later connect real collectors and durable-memory cues to initiative.
- The same recent session context can inform bounded proactive observations.
- Unchanged context causes at most one provider consideration rather than recurring polling.
- Game Mode and Disabled remain hard suppressors.
- Persistent memory, richer context collectors, and user-facing initiative settings remain separate future work.
