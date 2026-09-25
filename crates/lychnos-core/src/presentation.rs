//! Read-only state projection for Lychnos presentation layers.
//!
//! Presentation data intentionally contains no approval grants, runtime
//! controllers, executors, or mutation handles. A UI may render this state,
//! but it does not gain authority by receiving it.

use crate::{
    action::{ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability},
    diagnostics::{DiagnosticLevel, DiagnosticRecord},
    executor::MockRunningWorkState,
    runtime::RuntimeMode,
};

/// UI-facing summary of one action awaiting explicit approval.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingApprovalPresentation {
    pub action_id: ActionId,
    pub kind: ActionKind,
    pub capability: Capability,
    pub impact: ActionImpact,
    pub risk: ActionRisk,
    pub reason: String,
}

impl From<&ActionProposal> for PendingApprovalPresentation {
    fn from(proposal: &ActionProposal) -> Self {
        Self {
            action_id: proposal.id.clone(),
            kind: proposal.kind.clone(),
            capability: proposal.capability.clone(),
            impact: proposal.impact,
            risk: proposal.risk,
            reason: proposal.reason.clone(),
        }
    }
}

/// UI-facing summary of one tracked simulation-only work item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedWorkPresentation {
    pub action_id: ActionId,
    pub state: MockRunningWorkState,
    pub terminal: bool,
    pub cooperation_pending: bool,
}

impl TrackedWorkPresentation {
    #[must_use]
    pub fn new(action_id: ActionId, state: MockRunningWorkState) -> Self {
        Self {
            action_id,
            state,
            terminal: state.is_terminal(),
            cooperation_pending: state.cooperation_pending(),
        }
    }
}

/// UI-facing copy of one ordinary diagnostic record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticPresentation {
    pub level: DiagnosticLevel,
    pub component: String,
    pub message: String,
}

impl From<&DiagnosticRecord> for DiagnosticPresentation {
    fn from(record: &DiagnosticRecord) -> Self {
        Self {
            level: record.level,
            component: record.component.clone(),
            message: record.message.clone(),
        }
    }
}

/// Read-only snapshot consumed by a future desktop or portable presentation.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionPresentationState {
    pub runtime_mode: RuntimeMode,
    pub diagnostics_enabled: bool,
    pub pending_approvals: Vec<PendingApprovalPresentation>,
    pub tracked_work: Vec<TrackedWorkPresentation>,
    pub latest_diagnostic: Option<DiagnosticPresentation>,
}

impl CompanionPresentationState {
    #[must_use]
    pub fn pending_approval_count(&self) -> usize {
        self.pending_approvals.len()
    }

    #[must_use]
    pub fn cooperation_pending_count(&self) -> usize {
        self.tracked_work
            .iter()
            .filter(|work| work.cooperation_pending)
            .count()
    }

    #[must_use]
    pub fn terminal_work_count(&self) -> usize {
        self.tracked_work
            .iter()
            .filter(|work| work.terminal)
            .count()
    }

    #[must_use]
    pub fn latest_diagnostic_level(&self) -> Option<DiagnosticLevel> {
        self.latest_diagnostic.as_ref().map(|record| record.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_approval_projection_omits_execution_authority() {
        let proposal = ActionProposal::new(
            ActionId::new("action-present"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::Moderate,
            "Write one file",
            "test",
        );

        let projected = PendingApprovalPresentation::from(&proposal);

        assert_eq!(projected.action_id, proposal.id);
        assert_eq!(projected.kind, proposal.kind);
        assert_eq!(projected.capability, proposal.capability);
        assert_eq!(projected.reason, "Write one file");
    }

    #[test]
    fn tracked_work_projection_derives_terminal_and_cooperation_flags() {
        let waiting = TrackedWorkPresentation::new(
            ActionId::new("work-waiting"),
            MockRunningWorkState::PauseRequested,
        );
        assert!(waiting.cooperation_pending);
        assert!(!waiting.terminal);

        let completed = TrackedWorkPresentation::new(
            ActionId::new("work-completed"),
            MockRunningWorkState::Completed,
        );
        assert!(!completed.cooperation_pending);
        assert!(completed.terminal);
    }

    #[test]
    fn presentation_state_reports_counts_without_mutation_handles() {
        let state = CompanionPresentationState {
            runtime_mode: RuntimeMode::GameMode,
            diagnostics_enabled: true,
            pending_approvals: Vec::new(),
            tracked_work: vec![
                TrackedWorkPresentation::new(
                    ActionId::new("work-1"),
                    MockRunningWorkState::PauseRequested,
                ),
                TrackedWorkPresentation::new(
                    ActionId::new("work-2"),
                    MockRunningWorkState::Completed,
                ),
            ],
            latest_diagnostic: Some(DiagnosticPresentation {
                level: DiagnosticLevel::Warning,
                component: "runtime".into(),
                message: "example".into(),
            }),
        };

        assert_eq!(state.pending_approval_count(), 0);
        assert_eq!(state.cooperation_pending_count(), 1);
        assert_eq!(state.terminal_work_count(), 1);
        assert_eq!(
            state.latest_diagnostic_level(),
            Some(DiagnosticLevel::Warning)
        );
    }
}
