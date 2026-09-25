//! Machine-independent event collector boundaries for Lychnos.

use std::{collections::VecDeque, convert::Infallible, fmt};

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

/// Deterministic error used by the scripted collector for failure injection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectedCollectorError {
    message: String,
}

impl InjectedCollectorError {
    /// Creates one deterministic injected collector failure.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the injected failure message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InjectedCollectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for InjectedCollectorError {}

/// One deterministic step in a scripted collector scenario.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptedCollectorStep {
    /// Return one normalized event.
    Event(Event),

    /// Report that no event is currently available.
    Empty,

    /// Return one injected collector failure.
    Error(InjectedCollectorError),
}

/// Deterministic collector capable of replaying events, empty polls, and
/// injected failures in an exact order.
///
/// This exists only for simulation and testing. It performs no operating-system
/// observation.
#[derive(Debug, Default)]
pub struct ScriptedCollector {
    steps: VecDeque<ScriptedCollectorStep>,
}

impl ScriptedCollector {
    /// Creates an empty scripted collector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            steps: VecDeque::new(),
        }
    }

    /// Creates a scripted collector from an exact ordered sequence.
    #[must_use]
    pub fn from_steps(steps: impl IntoIterator<Item = ScriptedCollectorStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    /// Appends one deterministic scenario step.
    pub fn push(&mut self, step: ScriptedCollectorStep) {
        self.steps.push_back(step);
    }

    /// Returns the number of scripted steps still waiting.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.steps.len()
    }

    /// Returns whether the scripted scenario is exhausted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Collector for ScriptedCollector {
    type Error = InjectedCollectorError;

    fn collect(&mut self) -> Result<Option<Event>, Self::Error> {
        match self.steps.pop_front() {
            Some(ScriptedCollectorStep::Event(event)) => Ok(Some(event)),
            Some(ScriptedCollectorStep::Empty) | None => Ok(None),
            Some(ScriptedCollectorStep::Error(error)) => Err(error),
        }
    }
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
    fn scripted_collector_replays_exact_failure_sequence() {
        let injected = InjectedCollectorError::new("simulated collector failure");

        let mut collector = ScriptedCollector::from_steps([
            ScriptedCollectorStep::Event(event("event-script-1", "script.first")),
            ScriptedCollectorStep::Empty,
            ScriptedCollectorStep::Error(injected.clone()),
            ScriptedCollectorStep::Event(event("event-script-2", "script.recovered")),
        ]);

        let first = collector
            .collect()
            .expect("first scripted step should succeed")
            .expect("first scripted step should contain an event");

        assert_eq!(first.id.as_str(), "event-script-1");
        assert_eq!(collector.pending_len(), 3);

        assert!(
            collector
                .collect()
                .expect("empty scripted step should succeed")
                .is_none()
        );

        let error = collector
            .collect()
            .expect_err("third scripted step should inject a failure");

        assert_eq!(error, injected);
        assert_eq!(error.message(), "simulated collector failure");

        let recovered = collector
            .collect()
            .expect("collector should recover after injected failure")
            .expect("recovery step should contain an event");

        assert_eq!(recovered.id.as_str(), "event-script-2");
        assert_eq!(recovered.kind.as_str(), "script.recovered");
        assert!(collector.is_empty());
    }

    #[test]
    fn scripted_collector_can_be_built_incrementally() {
        let mut collector = ScriptedCollector::new();

        collector.push(ScriptedCollectorStep::Empty);
        collector.push(ScriptedCollectorStep::Event(event(
            "event-later",
            "script.later",
        )));

        assert_eq!(collector.pending_len(), 2);

        assert!(
            collector
                .collect()
                .expect("empty step should not fail")
                .is_none()
        );

        let event = collector
            .collect()
            .expect("event step should not fail")
            .expect("event should exist");

        assert_eq!(event.id.as_str(), "event-later");
        assert!(collector.is_empty());
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
