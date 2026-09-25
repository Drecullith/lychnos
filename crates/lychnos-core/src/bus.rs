//! Internal event and message transport.

use std::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender, TryRecvError},
};

use crate::event::Event;

/// Result of publishing one event to the current subscribers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishReport {
    /// Number of subscribers present when publication began.
    pub attempted: usize,

    /// Number of subscribers that received the event.
    pub delivered: usize,

    /// Number of disconnected subscribers removed during publication.
    pub disconnected: usize,
}

/// One subscription to the in-memory event bus.
#[derive(Debug)]
pub struct EventSubscription {
    receiver: Receiver<Event>,
}

impl EventSubscription {
    /// Attempts to receive the next event without blocking.
    pub fn try_recv(&self) -> Result<Event, TryRecvError> {
        self.receiver.try_recv()
    }
}

/// Simple in-process event bus used during the foundation phase.
///
/// This does not commit Lychnos to a permanent transport technology.
#[derive(Debug, Default)]
pub struct InMemoryEventBus {
    subscribers: Mutex<Vec<Sender<Event>>>,
}

impl InMemoryEventBus {
    /// Creates an empty event bus.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Creates a new subscription.
    #[must_use]
    pub fn subscribe(&self) -> EventSubscription {
        let (sender, receiver) = mpsc::channel();

        self.subscribers
            .lock()
            .expect("event bus subscriber lock poisoned")
            .push(sender);

        EventSubscription { receiver }
    }

    /// Publishes an event to all currently connected subscribers.
    pub fn publish(&self, event: Event) -> PublishReport {
        let mut subscribers = self
            .subscribers
            .lock()
            .expect("event bus subscriber lock poisoned");

        let attempted = subscribers.len();
        let mut delivered = 0;

        subscribers.retain(|subscriber| {
            if subscriber.send(event.clone()).is_ok() {
                delivered += 1;
                true
            } else {
                false
            }
        });

        PublishReport {
            attempted,
            delivered,
            disconnected: attempted - delivered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{
        Event, EventId, EventKind, EventPayload, EventSource, EventTimestamp, Sensitivity, Severity,
    };

    fn mock_event(id: &str) -> Event {
        Event {
            id: EventId::new(id),
            occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_000),
            source: EventSource::new("mock-source"),
            kind: EventKind::new("mock.event"),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        }
    }

    #[test]
    fn published_event_reaches_every_subscriber() {
        let bus = InMemoryEventBus::new();
        let first = bus.subscribe();
        let second = bus.subscribe();

        let report = bus.publish(mock_event("event-001"));

        assert_eq!(
            report,
            PublishReport {
                attempted: 2,
                delivered: 2,
                disconnected: 0,
            }
        );

        assert_eq!(
            first
                .try_recv()
                .expect("first subscriber should receive event")
                .id
                .as_str(),
            "event-001"
        );

        assert_eq!(
            second
                .try_recv()
                .expect("second subscriber should receive event")
                .id
                .as_str(),
            "event-001"
        );
    }

    #[test]
    fn disconnected_subscribers_are_removed() {
        let bus = InMemoryEventBus::new();
        let connected = bus.subscribe();
        let disconnected = bus.subscribe();

        drop(disconnected);

        let first_report = bus.publish(mock_event("event-001"));

        assert_eq!(
            first_report,
            PublishReport {
                attempted: 2,
                delivered: 1,
                disconnected: 1,
            }
        );

        connected
            .try_recv()
            .expect("connected subscriber should receive event");

        let second_report = bus.publish(mock_event("event-002"));

        assert_eq!(
            second_report,
            PublishReport {
                attempted: 1,
                delivered: 1,
                disconnected: 0,
            }
        );
    }
}
