# ADR 0043: Local Push-to-Talk Speech Recognition

## Status

Accepted.

## Context

Push-to-talk capture must become a real Lychnos conversation path without sending raw microphone audio to a model provider or making the shell responsible for speech recognition.

The first Omarchy prototype also exposed several lifecycle risks: stale capture requests, multiple runtimes consuming one inbox, recorder shutdown races, automatic source-port changes, and Whisper non-speech labels such as `(upbeat music)`.

## Decision

The installed user-level runtime owns microphone capture lifecycle and local speech recognition.

The voice path is:

1. the shell emits a versioned StartPushToTalk request;
2. the runtime selects the normalized input and applies optional machine-local source-port/volume settings;
3. PipeWire capture writes a temporary 16 kHz mono WAV beneath `$XDG_RUNTIME_DIR/lychnos/audio/`;
4. the runtime publishes Started/Transcribing/Transcribed/Failed status back to the shell;
5. local whisper.cpp transcribes the finalized WAV behind the provider-neutral `SpeechToTextProvider` boundary;
6. the transcript becomes a normal `ConversationRequest` with `InteractionSource::PushToTalk`;
7. the normal Lychnos conversation provider produces the reply;
8. the temporary WAV is deleted after transcription.

The runtime rejects stale stop requests, enforces a 60-second PTT limit, clears stale voice-control files at startup, and bounds recorder shutdown.

The launcher refuses to start the installed runtime when another Lychnos runtime process is already present.

Machine-specific audio routing may be configured in `~/.config/lychnos/voice.env`. These settings are exported to the runtime and applied immediately before explicit capture.

whisper.cpp runs locally with the multilingual base model. Non-speech token suppression is enabled, and pure annotation outputs such as `[Music]` or `(upbeat music)` are treated as no recognized speech rather than user text.

## Consequences

- Raw PTT audio stays local by default.
- Voice and typed messages share one conversation/persona path.
- The UI reflects runtime-confirmed capture state instead of inferring success from mouse events.
- Temporary voice recordings do not accumulate by default.
- Machine-specific routing does not leak into the platform-independent core.
- Wake-word capture can reuse the same STT and conversation path later.
- A future STT engine can replace whisper.cpp without changing Lychnos interaction semantics.
