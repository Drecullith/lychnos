//! Explicit user-approval primitives.
//!
//! Approval grants are bound to the complete structured action proposal rather
//! than only its action ID. This prevents an approval for one proposal from
//! being reused for a modified proposal that happens to carry the same ID.

use crate::action::{ActionId, ActionProposal};

/// One explicit approval for one exact action proposal.
///
/// The grant intentionally does not implement `Clone`. Execution boundaries
/// consume it by value so one grant represents one approval attempt.
///
/// Grants are currently created only inside `lychnos-core`. A trusted runtime
/// approval boundary will become the public issuance path.
#[derive(Debug, PartialEq)]
pub struct ApprovalGrant {
    proposal: ActionProposal,
    approved_by: String,
}

impl ApprovalGrant {
    /// Creates an approval grant for one exact proposal.
    ///
    /// This remains crate-private so arbitrary external components cannot mint
    /// approval grants directly.
    #[must_use]
    pub(crate) fn new(proposal: &ActionProposal, approved_by: impl Into<String>) -> Self {
        Self {
            proposal: proposal.clone(),
            approved_by: approved_by.into(),
        }
    }

    /// Returns whether this grant applies to the complete supplied proposal.
    #[must_use]
    pub fn matches(&self, proposal: &ActionProposal) -> bool {
        &self.proposal == proposal
    }

    /// Returns the approved action identifier.
    #[must_use]
    pub fn action_id(&self) -> &ActionId {
        &self.proposal.id
    }

    /// Returns the actor that explicitly approved the proposal.
    #[must_use]
    pub fn approved_by(&self) -> &str {
        &self.approved_by
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{
        ActionImpact, ActionKind, ActionParameters, ActionRisk, ActionValue, Capability,
    };

    fn proposal() -> ActionProposal {
        ActionProposal::new(
            ActionId::new("action-001"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::Moderate,
            "Write a configuration file",
            "test-analyzer",
        )
        .with_parameters(
            ActionParameters::new()
                .with_field("path", ActionValue::Text("/tmp/example".into()))
                .with_field("contents", ActionValue::Text("original".into())),
        )
    }

    #[test]
    fn grant_matches_the_exact_proposal() {
        let proposal = proposal();
        let grant = ApprovalGrant::new(&proposal, "user");

        assert!(grant.matches(&proposal));
        assert_eq!(grant.action_id().as_str(), "action-001");
        assert_eq!(grant.approved_by(), "user");
    }

    #[test]
    fn same_action_id_with_changed_parameters_does_not_match() {
        let original = proposal();
        let grant = ApprovalGrant::new(&original, "user");

        let changed = ActionProposal::new(
            ActionId::new("action-001"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::Moderate,
            "Write a configuration file",
            "test-analyzer",
        )
        .with_parameters(
            ActionParameters::new()
                .with_field("path", ActionValue::Text("/tmp/example".into()))
                .with_field("contents", ActionValue::Text("changed".into())),
        );

        assert!(!grant.matches(&changed));
    }

    #[test]
    fn same_action_id_with_changed_metadata_does_not_match() {
        let original = proposal();
        let grant = ApprovalGrant::new(&original, "user");

        let changed = ActionProposal::new(
            ActionId::new("action-001"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::Destructive,
            ActionRisk::Critical,
            "A more dangerous operation",
            "test-analyzer",
        )
        .with_parameters(original.parameters.clone());

        assert!(!grant.matches(&changed));
    }
}
