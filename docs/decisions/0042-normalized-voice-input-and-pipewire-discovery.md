# ADR 0042: Normalized Voice Input and PipeWire Discovery

## Status

Accepted.

## Context

Lychnos needs push-to-talk, wake-word, and voice-session interaction without making the platform-independent core depend on PipeWire, ALSA, or Omarchy.

Microphone discovery must also remain distinct from microphone capture so simply launching Lychnos does not begin recording.

## Decision

The core owns normalized voice vocabulary only:

- `AudioInputDeviceId`;
- `AudioInputDevice`;
- `VoiceActivationMode` with Push-to-Talk, Wake Word, and Voice Session.

The Omarchy runtime adapter discovers microphone/input devices through structured `pw-dump` data and identifies the current default source with `wpctl inspect @DEFAULT_AUDIO_SOURCE@`.

Discovery does not open or record the microphone.

The runtime reports discovered inputs at startup for diagnostics. Capture will be activated separately and explicitly by the voice interaction path.

## Consequences

- Platform-specific audio APIs remain outside `lychnos-core`.
- Device discovery can evolve independently on other operating systems.
- Launching Lychnos does not imply microphone capture.
- Push-to-talk can be implemented as the first capture trigger while wake-word and voice-session modes reuse the same normalized device model.
