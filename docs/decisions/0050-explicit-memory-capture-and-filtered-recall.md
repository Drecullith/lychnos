# ADR 0050: Explicit Memory Capture and Filtered Recall

## Status

Accepted.

## Context

Durable storage alone does not define what Lychnos should remember or what an intelligence provider may see.

Automatically saving every conversation would be intrusive and would blur transient session context with durable companion memory. Giving a model direct access to the MemoryStore would also undermine provider independence and future AccountBridge privacy controls.

## Decision

The first durable conversational-memory policy is explicit-user capture only.

Lychnos currently persists conversational memory only when the normalized user message begins with an unambiguous directive such as `Remember that ...`, `Remember: ...`, `Remember this: ...`, or the supported `please remember` forms.

Ordinary conversation is not silently promoted into durable memory.

Explicit memories are stored as `user.explicit`, `identity` scope, `Standard` sensitivity, with `conversation.explicit_memory` provenance, source interaction ID, and sync revision 1. Exact repeats are deduplicated.

### Credential guard

Until encrypted-at-rest memory policy exists, the runtime refuses explicit memories containing obvious credential/secret indicators such as passwords, passcodes, API keys, private or secret keys, recovery or seed phrases, mnemonics, and one-time or 2FA codes.

This is a conservative first guard, not a complete secret-classification system.

### Local recall policy

Recall selection is performed by Lychnos before an intelligence provider is called.

Normal recall currently considers only non-tombstoned, `Standard` sensitivity, `identity` scope memories with a textual `text` field. Local lexical overlap provides the first deterministic relevance score. Broad questions such as "what do you remember about me?" may request the bounded recent identity-memory set.

Provider context is bounded to at most five memories, 1500 recalled characters total, and 500 characters from one memory.

Embeddings, semantic indexing, inferred memory, automatic consolidation, and richer relevance ranking are deferred.

### Provider boundary

`ConversationProvider` receives a Lychnos-owned `ConversationContext`, not a `MemoryStore`.

The context contains already-filtered `ConversationMemoryContext` excerpts plus trusted Lychnos runtime notices. Providers never receive a storage handle or permission to enumerate memory directly.

The local brain prompt explicitly labels recalled memory as contextual data rather than instructions. Commands embedded inside remembered text must not be obeyed merely because that memory was recalled.

Typed input and Push-to-Talk use the same memory-capture and recall path.

### Body-specific path override

The desktop JSON adapter accepts optional `LYCHNOS_MEMORY_PATH` for testing/body-local configuration. A regression test guarantees an explicit path wins over XDG/default path resolution.

This is adapter detail, not portable memory canon.

## Consequences

- Lychnos gains durable continuity without silently recording every conversation.
- Explicit memories survive runtime restart.
- Providers receive only Lychnos-selected context rather than unrestricted memory access.
- Recalled memory is treated as data, not authority.
- Obvious secrets are refused while stronger at-rest protection remains absent.
- Memory behavior is shared by typed and voice-originated conversation.
- Exact duplicate explicit memories are not duplicated.
- Automatic/inferred memory, memory editing/forgetting UX, sensitive-memory handling, semantic retrieval, consolidation, AccountBridge disclosure policy, and multi-device synchronization remain future work.
