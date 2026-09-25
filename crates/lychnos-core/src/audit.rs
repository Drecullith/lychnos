//! Security-relevant audit records and interfaces.

use std::{collections::BTreeMap, convert::Infallible};

use crate::{action::ActionId, event::EventId};

/// Opaque identifier for one audit record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuditId(String);

impl AuditId {
    /// Creates an audit identifier.
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

/// Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuditTimestamp(u64);

impl AuditTimestamp {
    /// Creates an audit timestamp from Unix epoch milliseconds.
    #[must_use]
    pub const fn from_unix_millis(value: u64) -> Self {
        Self(value)
    }

    /// Returns the timestamp as Unix epoch milliseconds.
    #[must_use]
    pub const fn as_unix_millis(self) -> u64 {
        self.0
    }
}

/// Security-relevant occurrence recorded by Lychnos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditEventKind {
    EventObserved,
    ActionProposed,
    ActionApproved,
    ActionRejected,
    ActionCancelled,
    PermissionEvaluated,
    RuntimeModeChanged,
    ActionExecutionAttempted,
    ActionExecutionCompleted,
}

/// Scalar value stored in structured audit details.
#[derive(Debug, Clone, PartialEq)]
pub enum AuditValue {
    Text(String),
    Integer(i64),
    Unsigned(u64),
    Boolean(bool),
}

/// Structured additional information attached to an audit record.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AuditDetails {
    fields: BTreeMap<String, AuditValue>,
}

impl AuditDetails {
    /// Creates an empty collection of audit details.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Adds one structured detail.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: AuditValue) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Returns one detail by name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&AuditValue> {
        self.fields.get(key)
    }

    /// Returns the number of structured details.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether the record has no structured details.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// One immutable security audit record.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditRecord {
    pub id: AuditId,
    pub occurred_at: AuditTimestamp,
    pub kind: AuditEventKind,
    pub actor: String,
    pub message: String,
    pub event_id: Option<EventId>,
    pub action_id: Option<ActionId>,
    pub details: AuditDetails,
}

impl AuditRecord {
    /// Creates a security audit record.
    #[must_use]
    pub fn new(
        id: AuditId,
        occurred_at: AuditTimestamp,
        kind: AuditEventKind,
        actor: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            occurred_at,
            kind,
            actor: actor.into(),
            message: message.into(),
            event_id: None,
            action_id: None,
            details: AuditDetails::new(),
        }
    }

    /// Associates the audit record with a Lychnos event.
    #[must_use]
    pub fn with_event(mut self, event_id: EventId) -> Self {
        self.event_id = Some(event_id);
        self
    }

    /// Associates the audit record with an action proposal.
    #[must_use]
    pub fn with_action(mut self, action_id: ActionId) -> Self {
        self.action_id = Some(action_id);
        self
    }

    /// Adds structured audit details.
    #[must_use]
    pub fn with_details(mut self, details: AuditDetails) -> Self {
        self.details = details;
        self
    }
}

/// Destination for append-only security audit records.
pub trait AuditSink {
    type Error;

    /// Appends one audit record.
    fn append(&mut self, record: AuditRecord) -> Result<(), Self::Error>;
}

/// In-memory append-only audit sink used during the foundation phase.
#[derive(Debug, Default)]
pub struct InMemoryAuditLog {
    records: Vec<AuditRecord>,
}

impl InMemoryAuditLog {
    /// Creates an empty audit log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Returns every audit record in insertion order.
    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    /// Returns the number of audit records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether the audit log contains no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl AuditSink for InMemoryAuditLog {
    type Error = Infallible;

    fn append(&mut self, record: AuditRecord) -> Result<(), Self::Error> {
        self.records.push(record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_record_preserves_security_context() {
        let record = AuditRecord::new(
            AuditId::new("audit-001"),
            AuditTimestamp::from_unix_millis(1_800_000_000_000),
            AuditEventKind::PermissionEvaluated,
            "permission-policy",
            "Action requires user approval",
        )
        .with_event(EventId::new("event-001"))
        .with_action(ActionId::new("action-001"))
        .with_details(
            AuditDetails::new()
                .with_field(
                    "decision",
                    AuditValue::Text("requires_user_approval".into()),
                )
                .with_field("interactive", AuditValue::Boolean(true)),
        );

        assert_eq!(record.id.as_str(), "audit-001");
        assert_eq!(record.actor, "permission-policy");
        assert_eq!(
            record.event_id.as_ref().map(EventId::as_str),
            Some("event-001")
        );
        assert_eq!(
            record.action_id.as_ref().map(ActionId::as_str),
            Some("action-001")
        );
        assert_eq!(record.details.len(), 2);
    }

    #[test]
    fn in_memory_audit_log_is_append_only_through_its_public_api() {
        let mut log = InMemoryAuditLog::new();

        log.append(AuditRecord::new(
            AuditId::new("audit-001"),
            AuditTimestamp::from_unix_millis(1),
            AuditEventKind::EventObserved,
            "test",
            "First record",
        ))
        .expect("in-memory append cannot fail");

        log.append(AuditRecord::new(
            AuditId::new("audit-002"),
            AuditTimestamp::from_unix_millis(2),
            AuditEventKind::ActionProposed,
            "test",
            "Second record",
        ))
        .expect("in-memory append cannot fail");

        assert_eq!(log.len(), 2);
        assert_eq!(log.records()[0].id.as_str(), "audit-001");
        assert_eq!(log.records()[1].id.as_str(), "audit-002");
    }

    #[test]
    fn new_audit_log_is_empty() {
        let log = InMemoryAuditLog::new();

        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }
}
