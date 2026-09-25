//! Reusable identity and time providers for Lychnos.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    action::ActionId,
    audit::{AuditId, AuditTimestamp},
    event::{EventId, EventTimestamp},
    memory::{DeviceId, MemoryId, MemoryTimestamp},
};

/// Category of identifier requested from an ID provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    Event,
    Action,
    Audit,
    Memory,
    Device,
}

impl IdKind {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Action => "action",
            Self::Audit => "audit",
            Self::Memory => "memory",
            Self::Device => "device",
        }
    }
}

/// Source of Lychnos identifiers.
///
/// Concrete providers decide how identifiers are generated.
pub trait IdProvider {
    /// Generates the next raw identifier for a category.
    fn next_id(&mut self, kind: IdKind) -> String;

    /// Generates an event identifier.
    fn next_event_id(&mut self) -> EventId {
        EventId::new(self.next_id(IdKind::Event))
    }

    /// Generates an action identifier.
    fn next_action_id(&mut self) -> ActionId {
        ActionId::new(self.next_id(IdKind::Action))
    }

    /// Generates an audit identifier.
    fn next_audit_id(&mut self) -> AuditId {
        AuditId::new(self.next_id(IdKind::Audit))
    }

    /// Generates a memory identifier.
    fn next_memory_id(&mut self) -> MemoryId {
        MemoryId::new(self.next_id(IdKind::Memory))
    }

    /// Generates a device identifier.
    fn next_device_id(&mut self) -> DeviceId {
        DeviceId::new(self.next_id(IdKind::Device))
    }
}

/// Deterministic local sequence used during the foundation phase.
///
/// This is useful for tests and simulations. It is not intended to provide
/// globally unique IDs across multiple devices.
#[derive(Debug, Clone)]
pub struct SequenceIdProvider {
    next: u64,
}

impl Default for SequenceIdProvider {
    fn default() -> Self {
        Self::new(1)
    }
}

impl SequenceIdProvider {
    /// Creates a sequence provider beginning at the supplied value.
    #[must_use]
    pub const fn new(start: u64) -> Self {
        Self { next: start }
    }
}

impl IdProvider for SequenceIdProvider {
    fn next_id(&mut self, kind: IdKind) -> String {
        let value = self.next;

        self.next = self
            .next
            .checked_add(1)
            .expect("Lychnos sequence ID provider exhausted");

        format!("{}-{value:06}", kind.prefix())
    }
}

/// Source of wall-clock timestamps.
pub trait TimeProvider {
    /// Returns milliseconds since the Unix epoch.
    fn now_unix_millis(&self) -> u64;

    /// Returns the current time as an event timestamp.
    fn event_timestamp(&self) -> EventTimestamp {
        EventTimestamp::from_unix_millis(self.now_unix_millis())
    }

    /// Returns the current time as an audit timestamp.
    fn audit_timestamp(&self) -> AuditTimestamp {
        AuditTimestamp::from_unix_millis(self.now_unix_millis())
    }

    /// Returns the current time as a memory timestamp.
    fn memory_timestamp(&self) -> MemoryTimestamp {
        MemoryTimestamp::from_unix_millis(self.now_unix_millis())
    }
}

/// Deterministic clock for tests and simulations.
#[derive(Debug, Clone, Copy)]
pub struct FixedTimeProvider {
    unix_millis: u64,
}

impl FixedTimeProvider {
    /// Creates a fixed clock.
    #[must_use]
    pub const fn new(unix_millis: u64) -> Self {
        Self { unix_millis }
    }
}

impl TimeProvider for FixedTimeProvider {
    fn now_unix_millis(&self) -> u64 {
        self.unix_millis
    }
}

/// Wall clock backed by the operating system.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now_unix_millis(&self) -> u64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_provider_generates_typed_ids() {
        let mut provider = SequenceIdProvider::default();

        assert_eq!(provider.next_event_id().as_str(), "event-000001");
        assert_eq!(provider.next_action_id().as_str(), "action-000002");
        assert_eq!(provider.next_audit_id().as_str(), "audit-000003");
        assert_eq!(provider.next_memory_id().as_str(), "memory-000004");
        assert_eq!(provider.next_device_id().as_str(), "device-000005");
    }

    #[test]
    fn sequence_provider_can_start_at_known_value() {
        let mut provider = SequenceIdProvider::new(42);

        assert_eq!(provider.next_event_id().as_str(), "event-000042");
        assert_eq!(provider.next_audit_id().as_str(), "audit-000043");
    }

    #[test]
    fn fixed_time_provider_creates_typed_timestamps() {
        let provider = FixedTimeProvider::new(1_800_000_000_123);

        assert_eq!(
            provider.event_timestamp().as_unix_millis(),
            1_800_000_000_123
        );

        assert_eq!(
            provider.audit_timestamp().as_unix_millis(),
            1_800_000_000_123
        );

        assert_eq!(
            provider.memory_timestamp().as_unix_millis(),
            1_800_000_000_123
        );
    }
}
