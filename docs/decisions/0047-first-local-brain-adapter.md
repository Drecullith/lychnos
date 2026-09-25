# ADR 0047: First LocalBrain Adapter on Omarchy

## Status

Accepted.

## Context

Lychnos now has capability-aware intelligence routing, but still needs a real no-subscription local intelligence provider.

The implementation must remain body-specific: a strong Omarchy desktop can run a larger model, while phones and Pocket Lychnos may require smaller models or different inference runtimes.

The local intelligence provider must preserve Lychnos-owned persona, memory, initiative policy, permissions, and speech paths.

## Decision

The first real LocalBrain adapter is implemented in the Omarchy runtime using a local llama.cpp server bound only to `127.0.0.1`.

The runtime:

- starts llama.cpp as a child process;
- supplies the machine-local model selection through `~/.config/lychnos/brain.env`;
- authenticates local HTTP traffic with an ephemeral in-memory API token passed through the child environment;
- waits for the local model server to become healthy;
- implements the existing provider-neutral `ConversationProvider` contract;
- also implements `InitiativeProvider` for future bounded proactive speech;
- strips model-internal `<think>...</think>` content before presentation;
- supports model-specific prompt suffixes such as Qwen3 `/no_think` without adding them to Lychnos core;
- falls back to the deterministic mock provider when a local brain cannot start.

The X1 Pro development body currently selects:

- runtime: llama.cpp Vulkan build;
- model class: Large;
- model: official `Qwen/Qwen3-8B-GGUF:Q4_K_M`;
- context: 8192 tokens;
- local endpoint: `127.0.0.1:18181`.

This model/runtime combination is not a universal Lychnos requirement.

The capability-aware desktop installer selects one of four reference classes:

- Tiny: Qwen3 0.6B Q8_0;
- Compact: Qwen3 1.7B Q8_0;
- Standard: Qwen3 4B Q4_K_M;
- Large: Qwen3 8B Q4_K_M.

All current reference models are official Apache-2.0 Qwen GGUF releases.

## Consequences

- Lychnos now has a fully local conversational brain that requires no paid API or subscription.
- Typed, push-to-talk, wake-word, and future voice-session paths can all reuse the same local intelligence provider.
- The same persona and permissions remain stable when another model or runtime replaces llama.cpp/Qwen.
- Phones, iOS, Android, Windows, macOS, Linux, and Pocket bodies may select different adapters/models while preserving the same core contracts.
- The local model server is not exposed on the LAN.
- The llama child is explicitly stopped by the Lychnos launcher before the runtime exits.
- Live proactive initiative still requires a conservative runtime scheduler and meaningful context sources.
