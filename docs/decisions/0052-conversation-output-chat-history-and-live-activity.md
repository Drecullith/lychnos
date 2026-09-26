# ADR 0052: Conversation Output Modes, Chat History, and Live Companion Activity

## Status

Accepted.

## Context

Lychnos already had working typed interaction, Push-to-Talk, local STT/TTS, and a real LocalBrain, but the desktop experience still exposed prototype seams.

The chat title could imply a mock provider even when the real local brain was active. Conversation status text repeatedly replaced prior turns, so scrollback was effectively lost. Typed input had no user-selectable spoken reply mode. The companion face also did not reliably reflect the live listening/thinking/speaking state.

These issues needed to be fixed before wake-word and hands-free voice sessions, because hands-free interaction depends on trustworthy presentation state and clear output-mode semantics.

## Decision

### Interaction output mode

`ConversationRequest` now carries `ConversationOutputMode`.

Supported modes:

- `TextOnly`
- `TextAndSpeech`

Default behavior is source-aware:

- Typed -> TextOnly
- PushToTalk -> TextAndSpeech
- WakeWord -> TextAndSpeech
- VoiceSession -> TextAndSpeech

Typed requests may explicitly override the default through `with_output_mode(...)`.

The desktop shell persists a `speak_typed_replies` preference and exposes a `Voice · ON/OFF` control in the chat header.

### Real provider labeling

The runtime projects the active intelligence label through read-only presentation state.

The shell displays:

- `CHAT · LOCAL BRAIN` for the active local llama.cpp brain;
- `CHAT · FALLBACK MOCK` only when the deterministic mock is genuinely in use;
- a connecting/provider label otherwise.

### Conversation history

The desktop chat keeps a bounded in-memory presentation history rather than replacing the transcript for every state transition.

Actual user and Lychnos turns append to the scrollable history. Temporary progress such as microphone startup, transcription, thinking, listening, or speaking is displayed separately as activity/status UI and does not destroy chat scrollback.

The current shell keeps at most 80 rendered history entries. This is presentation history only and is not durable Lychnos memory.

### Live companion activity

`CompanionPresentationState` now projects a read-only `CompanionActivity`:

- Idle
- Listening
- Thinking
- Speaking

The runtime updates this activity through real conversation/voice lifecycle transitions.

The TTS worker exposes whether speech is currently active so the runtime can return Speaking -> Idle when playback completes.

The desktop face prioritizes live conversational activity before ordinary idle/work expressions:

- Listening -> Listening expression
- Thinking -> Thinking expression
- Speaking -> Speaking expression
- Idle -> ordinary runtime/diagnostic/work expression rules

## Physical verification

On the Minisforum AI X1 Pro / Omarchy development body, the user physically verified:

- real chat title shows Local Brain rather than Local Mock;
- multiple typed turns remain visible and can be scrolled;
- Voice ON causes replies to typed messages to be spoken while still appearing as text;
- Push-to-Talk preserves chat history;
- facial expression changes track Listening, Thinking, Speaking, and return to normal.

## Consequences

- Typed and voice interactions now share explicit presentation-output semantics.
- Voice reply preference does not depend on whether the microphone was used.
- Chat behaves like an actual conversation instead of a transient debug panel.
- The shell reflects live runtime activity without gaining authority.
- Presentation history remains separate from durable Lychnos memory.
- Wake-word and hands-free voice-session work can now build on real Listening/Thinking/Speaking states instead of inventing another parallel UI path.
