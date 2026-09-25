//! Platform-independent events used inside Lychnos.

use std::collections::BTreeMap;

/// Opaque identifier for one Lychnos event.
///
/// The mechanism used to generate globally unique identifiers is intentionally
/// left undecided during the machine-independent foundation phase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(String);

impl EventId {
    /// Creates an event identifier from a caller-provided value.
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
pub struct EventTimestamp(u64);

impl EventTimestamp {
    /// Creates a timestamp from Unix epoch milliseconds.
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

/// Opaque name identifying the component that produced an event.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventSource(String);

impl EventSource {
    /// Creates an event source.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the source name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque normalized event kind.
///
/// Examples may eventually include names such as
/// `terminal.command_failed` or `runtime.mode_requested`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventKind(String);

impl EventKind {
    /// Creates an event kind.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the normalized event kind.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Importance of an event to Lychnos and the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Severity {
    /// Routine informational event.
    #[default]
    Info,

    /// Something unusual occurred and may deserve attention.
    Warning,

    /// An operation or component failed.
    Error,

    /// A severe condition requiring prompt attention.
    Critical,
}

/// Data-sensitivity classification carried with an event.
///
/// This classification does not itself decide whether data may leave the
/// machine. Future privacy policy will make that decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Sensitivity {
    /// Ordinary local event data.
    #[default]
    Standard,

    /// Event data that deserves additional privacy handling.
    Sensitive,

    /// Event data requiring the strongest local handling policy.
    Restricted,
}

/// One scalar value inside an event payload.
#[derive(Debug, Clone, PartialEq)]
pub enum EventValue {
    Text(String),
    Integer(i64),
    Unsigned(u64),
    Boolean(bool),
}

/// Structured payload associated with an event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventPayload {
    fields: BTreeMap<String, EventValue>,
}

impl EventPayload {
    /// Creates an empty event payload.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Inserts a field and returns the payload for convenient construction.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: EventValue) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Returns a payload field by name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&EventValue> {
        self.fields.get(key)
    }

    /// Returns the number of payload fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether the payload contains no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Canonical platform-independent event passed through Lychnos.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub id: EventId,
    pub occurred_at: EventTimestamp,
    pub source: EventSource,
    pub kind: EventKind,
    pub severity: Severity,
    pub sensitivity: Sensitivity,
    pub correlation_id: Option<EventId>,
    pub payload: EventPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_event_preserves_its_envelope() {
        let event = Event {
            id: EventId::new("event-001"),
            occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_000),
            source: EventSource::new("mock-terminal"),
            kind: EventKind::new("terminal.command_failed"),
            severity: Severity::Error,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new()
                .with_field("command", EventValue::Text("cargo test".into()))
                .with_field("exit_code", EventValue::Integer(101)),
        };

        assert_eq!(event.id.as_str(), "event-001");
        assert_eq!(event.source.as_str(), "mock-terminal");
        assert_eq!(event.kind.as_str(), "terminal.command_failed");
        assert_eq!(event.severity, Severity::Error);
        assert_eq!(event.payload.len(), 2);
        assert_eq!(
            event.payload.get("exit_code"),
            Some(&EventValue::Integer(101))
        );
    }

    #[test]
    fn empty_payload_reports_empty() {
        let payload = EventPayload::new();

        assert!(payload.is_empty());
        assert_eq!(payload.len(), 0);
    }
}
