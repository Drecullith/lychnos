# ADR 0040: Provider-Neutral Interaction and Persona

## Status

Accepted.

## Context

Lychnos needs typed conversation now and voice interaction later, but the companion's identity must not become owned by ChatGPT, a local LLM, or any other replaceable model provider.

Push-to-talk, wake-word, and typed input should also converge on one normalized conversation boundary instead of becoming separate AI integrations.

## Decision

The platform-independent core owns:

- a canonical `PersonaProfile`;
- stable persona identity separate from model providers;
- normalized `ConversationRequest` and `ConversationResponse` types;
- a versioned interaction wire envelope;
- explicit input provenance through `InteractionSource`;
- a replaceable `ConversationProvider` interface.

The initial canonical persona is `lychnos.default.v1` and defines Lychnos as a local-first ambient companion with warm, observant, concise, curious, dry-witted, and calm-under-pressure traits.

The interaction source vocabulary includes:

- `Typed`;
- `PushToTalk`;
- `WakeWord`;
- `VoiceSession`.

The current `MockConversationProvider` is deterministic and local. It exists only to prove the interaction path before a real AI provider adapter is selected.

## Consequences

- Lychnos' name, character, and operating principles survive AI-provider replacement.
- Typed and voice interactions can use one normalized request/response path.
- A provider adapter receives Lychnos persona/context rather than defining Lychnos identity.
- The initial mock provider is not presented as the final intelligence layer.
- Persistent conversation history, provider selection, prompt construction, token budgeting, and real model calls remain future work.
