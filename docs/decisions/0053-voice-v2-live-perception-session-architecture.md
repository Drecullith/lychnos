# ADR 0053: Voice V2 Live Perception and Session Architecture

## Status

Accepted.

## Context

The first hands-free voice prototype coupled microphone level gating, Whisper transcription, wake-name recognition, speech endpointing, reply playback, and follow-up timing too tightly.

That design produced several physically observed failure modes:

- wake detection was unreliable because STT also had to decide whether Lychnos was addressed;
- longer user turns could be cut off or run to the safety limit;
- reply completion and follow-up re-arming could race;
- background video or another speaker could be treated as the user's follow-up;
- duplicate/orphan perception processes could compete for the microphone after runtime restarts;
- proactive initiative could overwrite active conversation state and prevent follow-up re-arming.

The project also needs a path toward future Pocket Lychnos camera/vision perception without making Omarchy-specific microphone logic part of the portable core.

## Decision

### Core-owned live-session state

`lychnos-core` owns a deterministic live-session controller with explicit states:

`WakeArmed -> Listening -> Thinking -> Speaking -> FollowUp -> Listening/WakeArmed`

Adapters emit facts; they do not invent parallel session state.
Impossible transitions are rejected rather than guessed.

Audio and Vision are first-class perception modalities so future body adapters can share the same session vocabulary.

### Separate Omarchy perception process

The first body adapter is `lychnos-perception-omarchy`.

It is a separate runtime child process so native audio/model failures do not crash the Lychnos core/runtime and so Game Mode can later suspend or unload perception independently.

The adapter has no host-action authority and no AI/persona authority.

### One job per voice component

Voice V2 separates responsibilities:

- sherpa-onnx keyword spotting decides whether the configured wake name was detected;
- WebRTC VAD decides speech onset and end-of-turn;
- Whisper performs transcription only;
- LocalBrain/provider performs reasoning only;
- Piper/TTS performs speech output only;
- the runtime owns conversation/session lifecycle.

When Voice V2 is available, the older Whisper-gated Voice V1 ambient listener is disabled so there is one ambient microphone owner.

### Natural follow-up sessions

A wake phrase opens one bounded conversation session.

After Lychnos finishes playback, the runtime explicitly sends `ResponseFinished { follow_up: true }`, opening a no-wake follow-up window.
The session returns to wake-required mode when:

- the follow-up window expires;
- a configured clear sign-off phrase is spoken;
- the runtime/body is disabled;
- an unrecoverable turn failure closes the session.

### Per-session speaker isolation

The wake turn creates an ephemeral speaker embedding for the person who opened the session.

No persistent voiceprint is written to Lychnos memory.

No-wake follow-ups must match the live session speaker closely enough before they can reach Whisper or the LocalBrain.

The first physically calibrated threshold is configurable and currently defaults to cosine similarity `0.35`.

Rejected non-session speech returns to the bounded FollowUp state without becoming a user message.

### Conversation-state ownership

Initiative/proactive reasoning may run only while companion activity is `Idle`.

It must never overwrite Listening, Thinking, Speaking, or FollowUp lifecycle state.

This prevents long spoken replies from losing the speech-complete transition needed to re-arm follow-up.

### Managed child lifecycle

The perception adapter exits when its runtime control channel closes.

The runtime handles SIGINT/SIGTERM gracefully so managed LocalBrain and perception children receive normal Rust drop cleanup.
Normal Lychnos shutdown/restart must not leave stale llama.cpp servers or orphan perception listeners behind.

### Presentation

Voice V2 publishes normalized transcript/request identity so hands-free turns appear in the same `CHAT · LOCAL BRAIN` history as typed/PTT turns.

Presentation history remains separate from durable Lychnos memory.

## Physical Verification

On the Minisforum AI X1 Pro / Omarchy development body, the following were physically verified:

- natural wake phrase detection;
- inline wake such as `So, Lychnos, what are we doing today?`;
- wake acknowledgement;
- clean VAD endpointing and full Whisper transcription;
- Qwen local replies and Piper playback;
- no-wake natural follow-up;
- bounded return to WakeArmed;
- explicit session-closing phrases;
- chat mirroring for user and Lychnos voice turns;
- user follow-ups accepted around cosine similarity `0.375–0.461`;
- video/background voices rejected around `0.10–0.29` before Whisper/Qwen;
- only one perception child owns the ambient microphone;
- SIGTERM cleanly tears down runtime, llama.cpp, and perception together.

## Consequences

- The working wake path is now a stable architectural baseline rather than a prototype detail.
- Future TTS engines, voices, languages, and providers can be replaced without redesigning wake/VAD/session ownership.
- Future Linux, macOS, Windows, and Pocket body adapters can implement local perception behind the same core session vocabulary.
- Future camera/vision support can enter through the perception layer without granting sensor adapters execution authority.
- Speaker verification improves follow-up isolation without creating a permanent biometric database.
- Voice V1 remains useful as historical/prototype code until deliberately removed, but it is not the preferred ambient architecture.
