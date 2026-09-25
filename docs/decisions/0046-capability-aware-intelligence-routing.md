# ADR 0046: Capability-Aware Intelligence Routing Across Bodies

## Status

Accepted.

## Context

Lychnos must run across very different bodies: desktops, Windows/Linux/macOS machines, phones, and the future Pocket Lychnos device.

A local model that is appropriate for a 60 GiB desktop may be unusable on a phone or battery-powered portable body. Conversely, choosing one tiny model as the universal baseline would unnecessarily cripple powerful machines.

Lychnos also needs optional account-backed intelligence bridges while remaining fully usable without subscriptions or paid APIs.

## Decision

The core does not define one universal local model.

Instead, Lychnos owns a provider-independent intelligence router and normalized capability vocabulary:

- `BodyPlatform` describes broad bodies including Linux, Windows, macOS, Android, iOS, and portable embedded devices;
- `DeviceCapabilityProfile` records normalized memory, CPU, accelerator availability, and low-power status;
- `LocalBrainClass` defines Tiny, Compact, Standard, and Large resource envelopes without naming specific models;
- `LocalModelManifest` describes an installable model independently from GGUF, Core ML, ONNX, or another runtime format;
- `LocalBrainSelectionPolicy` makes a conservative capability-aware recommendation;
- `IntelligenceSource` distinguishes Local, AccountBridge, and optional ApiByok routes;
- `IntelligenceRouter` defaults to Local when no supported account bridge is connected.

Local intelligence is the no-subscription baseline.

A connected account bridge may be preferred by the user when an official subscription-backed mechanism exists. Optional BYOK/API adapters may exist, but they are not required for baseline Lychnos.

Platform adapters own actual hardware probing, model installation, acceleration choice, and runtime integration.

The first Omarchy/X1 Pro implementation may use llama.cpp and a desktop-appropriate model, but neither is a universal Lychnos requirement.

## Consequences

- Lychnos identity, memory, persona, initiative, permissions, and synchronization remain unchanged when moving between desktop, phone, or Pocket bodies.
- Powerful machines may use larger local models without making those models installation requirements for everyone.
- Phones and low-power portable bodies may use smaller models or platform-native inference runtimes.
- The local-brain implementation can vary per operating system while preserving one core conversation/initiative contract.
- Account-backed agents remain optional enhancements rather than a substitute for the free local baseline.
- Model manifests and capability selection can evolve independently from Lychnos identity.
