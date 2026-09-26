//! Bounded proactive initiative for Lychnos.
//!
//! An intelligence provider may propose something worth saying, but this module
//! owns the provider-independent gate that decides whether the thought may be
//! surfaced. Initiative never grants execution authority.

use serde::{Deserialize, Serialize};

use crate::{persona::PersonaProfile, runtime::RuntimeMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitiativeMode {
    Off,
    Quiet,
    Normal,
    Proactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitiativePriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitiativeTrigger {
    DiagnosticChange,
    PendingApproval,
    ContextChange,
    ConversationFollowUp,
    MemoryCue,
    ScheduledCheck,
    UserIdle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitiativeCandidate {
    pub trigger: InitiativeTrigger,
    pub priority: InitiativePriority,
    pub message: String,
    /// Brief, inspectable explanation for why the candidate was proposed.
    ///
    /// This is not private chain-of-thought and must remain suitable for logs
    /// or diagnostics.
    pub reason_summary: String,
}

impl InitiativeCandidate {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.message.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitiativeContext {
    pub trigger: InitiativeTrigger,
    pub runtime_mode: RuntimeMode,
    pub initiative_mode: InitiativeMode,
    pub milliseconds_since_user_interaction: u64,
    pub milliseconds_since_last_surface: Option<u64>,
    pub user_is_interacting: bool,
    pub has_pending_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitiativeDecision {
    Surface,
    Suppress(InitiativeSuppressionReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitiativeSuppressionReason {
    EmptyMessage,
    InitiativeDisabled,
    RuntimeDisabled,
    GameMode,
    UserIsInteracting,
    QuietModeLowPriority,
    Cooldown,
}

/// Provider-neutral policy gate for proactive speech/text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitiativePolicy {
    pub normal_cooldown_ms: u64,
    pub quiet_cooldown_ms: u64,
}

impl Default for InitiativePolicy {
    fn default() -> Self {
        Self {
            normal_cooldown_ms: 60_000,
            quiet_cooldown_ms: 180_000,
        }
    }
}

impl InitiativePolicy {
    #[must_use]
    pub fn evaluate(
        &self,
        context: &InitiativeContext,
        candidate: &InitiativeCandidate,
    ) -> InitiativeDecision {
        if candidate.is_empty() {
            return InitiativeDecision::Suppress(InitiativeSuppressionReason::EmptyMessage);
        }

        if context.initiative_mode == InitiativeMode::Off {
            return InitiativeDecision::Suppress(InitiativeSuppressionReason::InitiativeDisabled);
        }

        match context.runtime_mode {
            RuntimeMode::Disabled => {
                return InitiativeDecision::Suppress(InitiativeSuppressionReason::RuntimeDisabled);
            }
            RuntimeMode::GameMode => {
                return InitiativeDecision::Suppress(InitiativeSuppressionReason::GameMode);
            }
            RuntimeMode::Normal => {}
        }

        if context.user_is_interacting && candidate.priority < InitiativePriority::Critical {
            return InitiativeDecision::Suppress(InitiativeSuppressionReason::UserIsInteracting);
        }

        if context.initiative_mode == InitiativeMode::Quiet
            && candidate.priority < InitiativePriority::High
        {
            return InitiativeDecision::Suppress(InitiativeSuppressionReason::QuietModeLowPriority);
        }

        let cooldown_ms = match context.initiative_mode {
            InitiativeMode::Quiet => self.quiet_cooldown_ms,
            InitiativeMode::Normal | InitiativeMode::Proactive => self.normal_cooldown_ms,
            InitiativeMode::Off => 0,
        };

        if candidate.priority < InitiativePriority::Critical
            && context
                .milliseconds_since_last_surface
                .is_some_and(|elapsed| elapsed < cooldown_ms)
        {
            return InitiativeDecision::Suppress(InitiativeSuppressionReason::Cooldown);
        }

        InitiativeDecision::Surface
    }
}

/// Replaceable intelligence boundary for proposing proactive speech.
///
/// The provider proposes only text. The runtime policy gate above decides
/// whether it may be surfaced, and normal action/approval boundaries remain
/// entirely separate.
pub trait InitiativeProvider {
    type Error;

    fn propose(
        &self,
        persona: &PersonaProfile,
        context: &InitiativeContext,
    ) -> Result<Option<InitiativeCandidate>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> InitiativeContext {
        InitiativeContext {
            trigger: InitiativeTrigger::ContextChange,
            runtime_mode: RuntimeMode::Normal,
            initiative_mode: InitiativeMode::Normal,
            milliseconds_since_user_interaction: 120_000,
            milliseconds_since_last_surface: None,
            user_is_interacting: false,
            has_pending_approval: false,
        }
    }

    fn candidate(priority: InitiativePriority) -> InitiativeCandidate {
        InitiativeCandidate {
            trigger: InitiativeTrigger::ContextChange,
            priority,
            message: "Terminal three failed twice; want me to take a look?".into(),
            reason_summary: "Repeated terminal failure detected.".into(),
        }
    }

    #[test]
    fn normal_mode_allows_useful_candidate() {
        assert_eq!(
            InitiativePolicy::default()
                .evaluate(&context(), &candidate(InitiativePriority::Normal)),
            InitiativeDecision::Surface
        );
    }

    #[test]
    fn game_mode_suppresses_proactive_speech() {
        let mut context = context();
        context.runtime_mode = RuntimeMode::GameMode;

        assert_eq!(
            InitiativePolicy::default()
                .evaluate(&context, &candidate(InitiativePriority::Critical)),
            InitiativeDecision::Suppress(InitiativeSuppressionReason::GameMode)
        );
    }

    #[test]
    fn disabled_mode_suppresses_proactive_speech() {
        let mut context = context();
        context.runtime_mode = RuntimeMode::Disabled;

        assert_eq!(
            InitiativePolicy::default()
                .evaluate(&context, &candidate(InitiativePriority::Critical)),
            InitiativeDecision::Suppress(InitiativeSuppressionReason::RuntimeDisabled)
        );
    }

    #[test]
    fn quiet_mode_requires_high_priority() {
        let mut context = context();
        context.initiative_mode = InitiativeMode::Quiet;

        assert_eq!(
            InitiativePolicy::default().evaluate(&context, &candidate(InitiativePriority::Normal)),
            InitiativeDecision::Suppress(InitiativeSuppressionReason::QuietModeLowPriority)
        );
        assert_eq!(
            InitiativePolicy::default().evaluate(&context, &candidate(InitiativePriority::High)),
            InitiativeDecision::Surface
        );
    }

    #[test]
    fn cooldown_blocks_repetitive_noncritical_speech() {
        let mut context = context();
        context.milliseconds_since_last_surface = Some(5_000);

        assert_eq!(
            InitiativePolicy::default().evaluate(&context, &candidate(InitiativePriority::Normal)),
            InitiativeDecision::Suppress(InitiativeSuppressionReason::Cooldown)
        );
    }

    #[test]
    fn critical_candidate_can_bypass_interaction_and_cooldown() {
        let mut context = context();
        context.user_is_interacting = true;
        context.milliseconds_since_last_surface = Some(1_000);

        assert_eq!(
            InitiativePolicy::default()
                .evaluate(&context, &candidate(InitiativePriority::Critical)),
            InitiativeDecision::Surface
        );
    }
}
