//! Read-only state projection for Lychnos presentation layers.
//!
//! Presentation data intentionally contains no approval grants, runtime
//! controllers, executors, or mutation handles. A UI may render this state,
//! but it does not gain authority by receiving it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    action::{ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability},
    diagnostics::{DiagnosticLevel, DiagnosticRecord},
    executor::MockRunningWorkState,
    runtime::RuntimeMode,
};

/// UI-facing summary of one action awaiting explicit approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingApprovalPresentation {
    pub action_id: ActionId,
    /// Opaque digest binding this UI row to the exact in-memory proposal.
    pub proposal_binding: String,
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
            proposal_binding: proposal_binding(proposal),
            kind: proposal.kind.clone(),
            capability: proposal.capability.clone(),
            impact: proposal.impact,
            risk: proposal.risk,
            reason: proposal.reason.clone(),
        }
    }
}

fn proposal_binding(proposal: &ActionProposal) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{proposal:?}").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// User decision emitted by a presentation surface for one pending action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingApprovalDecision {
    Approve,
    Reject,
}

/// Authority-free user intent sent from a presentation surface to the runtime owner.
///
/// The opaque proposal binding prevents a stale UI decision from being applied to
/// a newer proposal that happens to reuse the same action ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingApprovalControlRequest {
    pub action_id: ActionId,
    pub proposal_binding: String,
    pub decision: PendingApprovalDecision,
}

impl PendingApprovalControlRequest {
    #[must_use]
    pub fn from_presentation(
        approval: &PendingApprovalPresentation,
        decision: PendingApprovalDecision,
    ) -> Self {
        Self {
            action_id: approval.action_id.clone(),
            proposal_binding: approval.proposal_binding.clone(),
            decision,
        }
    }

    #[must_use]
    pub fn matches_proposal(&self, proposal: &ActionProposal) -> bool {
        self.action_id == proposal.id && self.proposal_binding == proposal_binding(proposal)
    }
}

/// Current wire schema for presentation-to-runtime control requests.
pub const CONTROL_SCHEMA_VERSION: u32 = 1;

/// Versioned envelope for authority-free presentation control requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionControlEnvelope {
    pub schema_version: u32,
    pub request: PendingApprovalControlRequest,
}

impl CompanionControlEnvelope {
    #[must_use]
    pub const fn new(request: PendingApprovalControlRequest) -> Self {
        Self {
            schema_version: CONTROL_SCHEMA_VERSION,
            request,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, ControlEnvelopeError> {
        let envelope: Self = serde_json::from_str(input)
            .map_err(|error| ControlEnvelopeError::Parse(error.to_string()))?;

        if envelope.schema_version != CONTROL_SCHEMA_VERSION {
            return Err(ControlEnvelopeError::UnsupportedSchemaVersion {
                found: envelope.schema_version,
                supported: CONTROL_SCHEMA_VERSION,
            });
        }

        Ok(envelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEnvelopeError {
    Parse(String),
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

/// UI-facing summary of one tracked simulation-only work item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Current conversational/activity state projected to presentation layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompanionActivity {
    #[default]
    Idle,
    Listening,
    Thinking,
    Speaking,
}

/// Read-only snapshot consumed by a future desktop or portable presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompanionPresentationState {
    pub runtime_mode: RuntimeMode,
    pub diagnostics_enabled: bool,
    pub pending_approvals: Vec<PendingApprovalPresentation>,
    pub tracked_work: Vec<TrackedWorkPresentation>,
    pub latest_diagnostic: Option<DiagnosticPresentation>,
    pub activity: CompanionActivity,
    pub intelligence_label: String,
}

/// Current wire schema for read-only companion presentation snapshots.
pub const PRESENTATION_SCHEMA_VERSION: u32 = 2;

/// Versioned, authority-free snapshot transported from the runtime owner to
/// presentation processes.
///
/// The envelope carries only CompanionPresentationState. It never contains
/// executors, approval grants, runtime controllers, or mutation handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionPresentationEnvelope {
    pub schema_version: u32,
    pub state: CompanionPresentationState,
}

impl CompanionPresentationEnvelope {
    #[must_use]
    pub const fn new(state: CompanionPresentationState) -> Self {
        Self {
            schema_version: PRESENTATION_SCHEMA_VERSION,
            state,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, PresentationEnvelopeError> {
        let envelope: Self = serde_json::from_str(input)
            .map_err(|error| PresentationEnvelopeError::Parse(error.to_string()))?;

        if envelope.schema_version != PRESENTATION_SCHEMA_VERSION {
            return Err(PresentationEnvelopeError::UnsupportedSchemaVersion {
                found: envelope.schema_version,
                supported: PRESENTATION_SCHEMA_VERSION,
            });
        }

        Ok(envelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationEnvelopeError {
    Parse(String),
    UnsupportedSchemaVersion { found: u32, supported: u32 },
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
    fn approval_control_request_round_trips_and_binds_exact_proposal() {
        let proposal = ActionProposal::new(
            ActionId::new("action-control"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::Moderate,
            "Write the requested file",
            "test",
        );
        let projected = PendingApprovalPresentation::from(&proposal);
        let request = PendingApprovalControlRequest::from_presentation(
            &projected,
            PendingApprovalDecision::Approve,
        );
        let envelope = CompanionControlEnvelope::new(request.clone());
        let json = envelope
            .to_json()
            .expect("control envelope should serialize");
        let decoded =
            CompanionControlEnvelope::from_json(&json).expect("control envelope should decode");

        assert_eq!(decoded.request, request);
        assert!(decoded.request.matches_proposal(&proposal));

        let changed = ActionProposal::new(
            ActionId::new("action-control"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::Moderate,
            "Write a different file",
            "test",
        );

        assert!(!decoded.request.matches_proposal(&changed));
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
    fn presentation_envelope_round_trips_without_authority() {
        let state = CompanionPresentationState {
            runtime_mode: RuntimeMode::GameMode,
            diagnostics_enabled: true,
            pending_approvals: vec![PendingApprovalPresentation {
                action_id: ActionId::new("action-wire"),
                proposal_binding: "binding-wire".into(),
                kind: ActionKind::new("file.write"),
                capability: Capability::new("file.write"),
                impact: ActionImpact::StateChanging,
                risk: ActionRisk::Moderate,
                reason: "Write one file".into(),
            }],
            tracked_work: vec![TrackedWorkPresentation::new(
                ActionId::new("work-wire"),
                MockRunningWorkState::PauseRequested,
            )],
            latest_diagnostic: Some(DiagnosticPresentation {
                level: DiagnosticLevel::Warning,
                component: "runtime".into(),
                message: "example warning".into(),
            }),
            activity: CompanionActivity::Idle,
            intelligence_label: "FOUNDATION".into(),
        };

        let envelope = CompanionPresentationEnvelope::new(state.clone());
        let json = envelope
            .to_json()
            .expect("presentation envelope should serialize");
        let decoded = CompanionPresentationEnvelope::from_json(&json)
            .expect("presentation envelope should decode");

        assert_eq!(decoded.schema_version, PRESENTATION_SCHEMA_VERSION);
        assert_eq!(decoded.state, state);
    }

    #[test]
    fn presentation_envelope_rejects_unknown_schema_version() {
        let state = CompanionPresentationState {
            runtime_mode: RuntimeMode::Normal,
            diagnostics_enabled: true,
            pending_approvals: Vec::new(),
            tracked_work: Vec::new(),
            latest_diagnostic: None,
            activity: CompanionActivity::Idle,
            intelligence_label: "FOUNDATION".into(),
        };
        let mut envelope = CompanionPresentationEnvelope::new(state);
        envelope.schema_version += 1;

        let json = envelope
            .to_json()
            .expect("presentation envelope should serialize");
        let error = CompanionPresentationEnvelope::from_json(&json)
            .expect_err("future schema must fail closed");

        assert_eq!(
            error,
            PresentationEnvelopeError::UnsupportedSchemaVersion {
                found: PRESENTATION_SCHEMA_VERSION + 1,
                supported: PRESENTATION_SCHEMA_VERSION,
            }
        );
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
            activity: CompanionActivity::Idle,
            intelligence_label: "FOUNDATION".into(),
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
