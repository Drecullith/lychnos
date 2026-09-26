//! Platform-independent live perception and conversation-session vocabulary.
//!
//! The core owns deterministic session state. Host-specific microphone,
//! wake-word, VAD, camera, and model-streaming implementations are adapters.
//! This lets the desktop and future portable body share one interaction model.

use serde::{Deserialize, Serialize};

/// Sensory channels that a Lychnos body may provide to a live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionModality {
    Audio,
    Vision,
}

/// Explicit state of one ambient conversation session.
///
/// Keeping this state in one controller prevents wake detection, VAD, speech
/// playback, and presentation code from each inventing their own idea of
/// whether Lychnos is currently listening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LiveSessionState {
    /// Ambient perception is disabled (for example Disabled/Game Mode policy).
    Disabled,
    /// Local wake detection is armed. No conversation is active yet.
    #[default]
    WakeArmed,
    /// A wake/follow-up turn is active and user speech is being captured.
    Listening,
    /// The completed user turn is being transcribed/reasoned about.
    Thinking,
    /// Lychnos is producing or playing the assistant response.
    Speaking,
    /// Reply finished; speech onset alone may start another turn.
    FollowUp,
}

/// External facts that move the session controller between states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveSessionEvent {
    Enable,
    Disable,
    WakeDetected,
    FollowUpSpeechDetected,
    FollowUpRejected,
    UtteranceFinalized,
    ResponseStarted,
    ResponseFinished { follow_up: bool },
    FollowUpSpeakerRejected { score_milli: u16 },
    FollowUpExpired,
}

/// Rejected transition. Adapters should log this instead of guessing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidLiveSessionTransition {
    pub state: LiveSessionState,
    pub event: LiveSessionEvent,
}

/// Single owner of the live-conversation state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LiveSessionController {
    state: LiveSessionState,
}

impl LiveSessionController {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: LiveSessionState::WakeArmed,
        }
    }

    #[must_use]
    pub const fn state(&self) -> LiveSessionState {
        self.state
    }

    pub fn apply(
        &mut self,
        event: LiveSessionEvent,
    ) -> Result<LiveSessionState, InvalidLiveSessionTransition> {
        use LiveSessionEvent as Event;
        use LiveSessionState as State;

        let next = match (self.state, event) {
            (_, Event::Disable) => State::Disabled,
            (State::Disabled, Event::Enable) => State::WakeArmed,

            (State::WakeArmed, Event::WakeDetected) => State::Listening,
            (State::Listening, Event::UtteranceFinalized) => State::Thinking,
            (State::Thinking, Event::ResponseStarted) => State::Speaking,
            (State::Speaking, Event::ResponseFinished { follow_up: true }) => State::FollowUp,
            (State::Speaking, Event::ResponseFinished { follow_up: false }) => State::WakeArmed,

            (State::FollowUp, Event::FollowUpSpeechDetected) => State::Listening,
            (State::Listening, Event::FollowUpRejected) => State::FollowUp,
            (State::FollowUp, Event::FollowUpExpired) => State::WakeArmed,

            // Idempotent enable while already available is useful during body
            // reconnects and does not alter an active conversation.
            (state, Event::Enable) if state != State::Disabled => state,

            _ => {
                return Err(InvalidLiveSessionTransition {
                    state: self.state,
                    event,
                });
            }
        };

        self.state = next;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_wake_reply_follow_up_cycle_is_explicit() {
        let mut session = LiveSessionController::new();

        assert_eq!(session.state(), LiveSessionState::WakeArmed);
        assert_eq!(
            session.apply(LiveSessionEvent::WakeDetected),
            Ok(LiveSessionState::Listening)
        );
        assert_eq!(
            session.apply(LiveSessionEvent::UtteranceFinalized),
            Ok(LiveSessionState::Thinking)
        );
        assert_eq!(
            session.apply(LiveSessionEvent::ResponseStarted),
            Ok(LiveSessionState::Speaking)
        );
        assert_eq!(
            session.apply(LiveSessionEvent::ResponseFinished { follow_up: true }),
            Ok(LiveSessionState::FollowUp)
        );
        assert_eq!(
            session.apply(LiveSessionEvent::FollowUpSpeechDetected),
            Ok(LiveSessionState::Listening)
        );
    }

    #[test]
    fn rejected_follow_up_returns_to_follow_up_window() {
        let mut session = LiveSessionController {
            state: LiveSessionState::Listening,
        };

        assert_eq!(
            session.apply(LiveSessionEvent::FollowUpRejected),
            Ok(LiveSessionState::FollowUp)
        );
    }

    #[test]
    fn follow_up_expiry_requires_wake_again() {
        let mut session = LiveSessionController {
            state: LiveSessionState::FollowUp,
        };

        assert_eq!(
            session.apply(LiveSessionEvent::FollowUpExpired),
            Ok(LiveSessionState::WakeArmed)
        );
    }

    #[test]
    fn reply_can_return_directly_to_wake_required() {
        let mut session = LiveSessionController {
            state: LiveSessionState::Speaking,
        };

        assert_eq!(
            session.apply(LiveSessionEvent::ResponseFinished { follow_up: false }),
            Ok(LiveSessionState::WakeArmed)
        );
    }

    #[test]
    fn disabled_state_requires_explicit_enable() {
        let mut session = LiveSessionController::new();
        assert_eq!(
            session.apply(LiveSessionEvent::Disable),
            Ok(LiveSessionState::Disabled)
        );
        assert!(session.apply(LiveSessionEvent::WakeDetected).is_err());
        assert_eq!(
            session.apply(LiveSessionEvent::Enable),
            Ok(LiveSessionState::WakeArmed)
        );
    }

    #[test]
    fn impossible_transition_is_rejected_instead_of_guessed() {
        let mut session = LiveSessionController::new();
        let error = session
            .apply(LiveSessionEvent::ResponseStarted)
            .expect_err("wake-armed session must not jump straight to speaking");

        assert_eq!(error.state, LiveSessionState::WakeArmed);
        assert_eq!(error.event, LiveSessionEvent::ResponseStarted);
        assert_eq!(session.state(), LiveSessionState::WakeArmed);
    }

    #[test]
    fn audio_and_vision_are_first_class_modalities() {
        assert_ne!(PerceptionModality::Audio, PerceptionModality::Vision);
    }
}

/// Current schema for body-adapter perception events and controls.
pub const PERCEPTION_SCHEMA_VERSION: u32 = 1;

/// Whether a finalized utterance began from a wake phrase or a no-wake follow-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionUtteranceSource {
    Wake,
    FollowUp,
}

/// Authority-free events emitted by a body-specific perception adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PerceptionEvent {
    Ready {
        wake_phrase: String,
        device: String,
    },
    State {
        state: LiveSessionState,
    },
    WakeDetected {
        keyword: String,
    },
    UtteranceFinalized {
        path: String,
        duration_ms: u64,
        source: PerceptionUtteranceSource,
    },
    FollowUpSpeakerRejected {
        score_milli: u16,
    },
    FollowUpExpired,
}

/// Runtime-to-adapter controls. These carry session lifecycle only; they never
/// grant host execution authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum PerceptionControl {
    Enable,
    Disable,
    ResponseStarted,
    ResponseFinished { follow_up: bool },
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn perception_event_round_trips_as_json() {
        let event = PerceptionEvent::UtteranceFinalized {
            path: "/tmp/turn.wav".into(),
            duration_ms: 1_250,
            source: PerceptionUtteranceSource::FollowUp,
        };
        let json = serde_json::to_string(&event).expect("event should serialize");
        let decoded: PerceptionEvent =
            serde_json::from_str(&json).expect("event should deserialize");
        assert_eq!(decoded, event);
    }

    #[test]
    fn perception_control_round_trips_as_json() {
        let control = PerceptionControl::ResponseFinished { follow_up: true };
        let json = serde_json::to_string(&control).expect("control should serialize");
        let decoded: PerceptionControl =
            serde_json::from_str(&json).expect("control should deserialize");
        assert_eq!(decoded, control);
    }
}
