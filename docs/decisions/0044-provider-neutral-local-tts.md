# ADR 0044: Provider-Neutral Local Text-to-Speech

## Status

Accepted.

## Context

Lychnos needs a stable spoken identity without coupling that identity to one text-to-speech engine or voice-model file.

Speech output must also not block the runtime's approval, interaction, or capture loop while audio is playing.

## Decision

The platform-independent core owns:

- a stable `SpeechVoiceId`;
- a provider-neutral `SpeechVoiceProfile`;
- `SpeechSynthesisRequest` and `SpeechSynthesisResult`;
- the replaceable `SpeechSynthesizer` boundary.

The canonical baseline voice identity is `lychnos.voice.default.v1`.

The initial Omarchy adapter uses local Piper TTS with a British male medium voice model as the first engine binding for that identity.

Voice-originated conversations currently use text + spoken output:

1. STT creates a normal `ConversationRequest`;
2. the conversation provider produces a normal Lychnos response;
3. the response is published to the shell;
4. the same response text is queued to a dedicated speech-output worker;
5. Piper synthesizes an ephemeral WAV;
6. PipeWire plays the WAV;
7. the synthesized WAV is deleted.

Speech output is serialized through one worker thread so replies do not overlap and runtime control processing remains responsive.

Typed chat remains text-only until explicit output-mode preferences are added.

## Consequences

- Lychnos' voice identity survives replacement of Piper or the current model file.
- Spoken output does not grant the TTS engine model/runtime authority.
- Voice replies can be changed or disabled independently from persona and conversation intelligence.
- Future voice choices, speaking-rate controls, output-device selection, and text/voice/both modes can be added without changing conversation semantics.
