//! Machine-independent event collector boundaries for Lychnos.

use std::{collections::VecDeque, convert::Infallible};

use crate::event::Event;

/// Source of already-normalized Lychnos events.
///
/// Platform-specific collectors may later observe terminals, journals,
/// services, hardware, or other sources, but the core only receives the
/// normalized `Event` boundary.
pub trait Collector {
    type Error;

    /// Collects the next available normalized event.
    ///
    /// `Ok(None)` means the collector currently has no event available.
    fn collect(&mut self) -> Result<Option<Event>, Self::Error>;
}

/// Deterministic FIFO collector used during the foundation phase.
///
/// It performs no operating-system observation. Tests and simulations feed
/// already-normalized events into it explicitly.
#[derive(Debug, Default)]
pub struct MockCollector {
    events: VecDeque<Event>,
}

impl MockCollector {
    /// Creates an empty mock collector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Creates a mock collector containing supplied events in FIFO order.
    #[must_use]
    pub fn from_events(events: impl IntoIterator<Item = Event>) -> Self {
        Self {
            events: events.into_iter().collect(),
        }
    }

    /// Queues one normalized event.
    pub fn push(&mut self, event: Event) {
        self.events.push_back(event);
    }

    /// Returns the number of queued events.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether no events are currently queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Collector for MockCollector {
    type Error = Infallible;

    fn collect(&mut self) -> Result<Option<Event>, Self::Error> {
        Ok(self.events.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{
        EventId, EventKind, EventPayload, EventSource, EventTimestamp, Sensitivity, Severity,
    };

    fn event(id: &str, kind: &str) -> Event {
        Event {
            id: EventId::new(id),
            occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_123),
            source: EventSource::new("mock-collector"),
            kind: EventKind::new(kind),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        }
    }

    #[test]
    fn mock_collector_returns_events_in_fifo_order() {
        let mut collector = MockCollector::from_events([
            event("event-001", "mock.first"),
            event("event-002", "mock.second"),
        ]);

        assert_eq!(collector.pending_len(), 2);

        let first = collector
            .collect()
            .expect("mock collection cannot fail")
            .expect("first event should exist");

        let second = collector
            .collect()
            .expect("mock collection cannot fail")
            .expect("second event should exist");

        assert_eq!(first.id.as_str(), "event-001");
        assert_eq!(first.kind.as_str(), "mock.first");
        assert_eq!(second.id.as_str(), "event-002");
        assert_eq!(second.kind.as_str(), "mock.second");
        assert!(collector.is_empty());
    }

    #[test]
    fn empty_mock_collector_returns_no_event() {
        let mut collector = MockCollector::new();

        let event = collector.collect().expect("mock collection cannot fail");

        assert!(event.is_none());
        assert_eq!(collector.pending_len(), 0);
    }
}
