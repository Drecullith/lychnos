//! Structured actions proposed by Lychnos.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::event::EventId;

/// Opaque identifier for one proposed action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActionId(String);

impl ActionId {
    /// Creates an action identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque normalized action kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActionKind(String);

impl ActionKind {
    /// Creates an action kind.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the action kind as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque capability required to perform an action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Capability(String);

impl Capability {
    /// Creates a capability identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the capability name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Effect an action may have on the machine or outside world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionImpact {
    /// Reads information without changing external state.
    ReadOnly,

    /// Changes local system or application state.
    StateChanging,

    /// Produces an externally visible effect.
    External,

    /// Requires elevated or otherwise privileged authority.
    Privileged,

    /// Can delete, overwrite, or irreversibly damage state.
    Destructive,
}

/// Estimated risk associated with an action proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionRisk {
    Low,
    Moderate,
    High,
    Critical,
}

/// Scalar value used as a structured action parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionValue {
    Text(String),
    Integer(i64),
    Unsigned(u64),
    Boolean(bool),
}

/// Structured parameters for an action proposal.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActionParameters {
    fields: BTreeMap<String, ActionValue>,
}

impl ActionParameters {
    /// Creates an empty parameter collection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Inserts a parameter.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: ActionValue) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Returns one parameter by name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ActionValue> {
        self.fields.get(key)
    }

    /// Returns whether there are no parameters.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// A proposed action that has not yet been authorized or executed.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionProposal {
    pub id: ActionId,
    pub kind: ActionKind,
    pub capability: Capability,
    pub impact: ActionImpact,
    pub risk: ActionRisk,
    pub parameters: ActionParameters,
    pub reason: String,
    pub proposer: String,
    pub source_event_id: Option<EventId>,
}

impl ActionProposal {
    /// Creates a structured action proposal.
    #[must_use]
    pub fn new(
        id: ActionId,
        kind: ActionKind,
        capability: Capability,
        impact: ActionImpact,
        risk: ActionRisk,
        reason: impl Into<String>,
        proposer: impl Into<String>,
    ) -> Self {
        Self {
            id,
            kind,
            capability,
            impact,
            risk,
            parameters: ActionParameters::new(),
            reason: reason.into(),
            proposer: proposer.into(),
            source_event_id: None,
        }
    }

    /// Adds structured parameters to the proposal.
    #[must_use]
    pub fn with_parameters(mut self, parameters: ActionParameters) -> Self {
        self.parameters = parameters;
        self
    }

    /// Associates the proposal with the event that caused it.
    #[must_use]
    pub fn with_source_event(mut self, event_id: EventId) -> Self {
        self.source_event_id = Some(event_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_preserves_structured_action_data() {
        let proposal = ActionProposal::new(
            ActionId::new("action-001"),
            ActionKind::new("package.inspect"),
            Capability::new("package.read"),
            ActionImpact::ReadOnly,
            ActionRisk::Low,
            "Inspect package metadata",
            "mock-analyzer",
        )
        .with_parameters(
            ActionParameters::new().with_field("package", ActionValue::Text("example".into())),
        )
        .with_source_event(EventId::new("event-001"));

        assert_eq!(proposal.id.as_str(), "action-001");
        assert_eq!(proposal.kind.as_str(), "package.inspect");
        assert_eq!(proposal.capability.as_str(), "package.read");
        assert_eq!(
            proposal.parameters.get("package"),
            Some(&ActionValue::Text("example".into()))
        );
        assert_eq!(
            proposal.source_event_id.as_ref().map(EventId::as_str),
            Some("event-001")
        );
    }
}
