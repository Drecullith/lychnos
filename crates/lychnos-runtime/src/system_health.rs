use std::{
    collections::BTreeSet,
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use lychnos_core::{
    collector::Collector,
    event::{
        Event, EventId, EventKind, EventPayload, EventSource, EventTimestamp, EventValue,
        Sensitivity, Severity,
    },
    initiative::InitiativeObservation,
};

const POLL_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub struct UserServiceHealthCollector {
    initialized: bool,
    last_failed: BTreeSet<String>,
    next_poll: Instant,
}

impl UserServiceHealthCollector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            initialized: false,
            last_failed: BTreeSet::new(),
            next_poll: Instant::now(),
        }
    }

    fn observe_failed_units(&mut self, failed: BTreeSet<String>) -> Option<Event> {
        let previous = std::mem::replace(&mut self.last_failed, failed.clone());

        if !self.initialized {
            self.initialized = true;
            if failed.is_empty() {
                return None;
            }

            return Some(build_health_event(
                "system.user_services.failed",
                Severity::Error,
                &failed,
                &failed,
                &BTreeSet::new(),
            ));
        }

        if failed == previous {
            return None;
        }

        let newly_failed: BTreeSet<_> = failed.difference(&previous).cloned().collect();
        let recovered: BTreeSet<_> = previous.difference(&failed).cloned().collect();

        if failed.is_empty() {
            Some(build_health_event(
                "system.user_services.recovered",
                Severity::Info,
                &failed,
                &newly_failed,
                &recovered,
            ))
        } else {
            Some(build_health_event(
                "system.user_services.failed_changed",
                Severity::Error,
                &failed,
                &newly_failed,
                &recovered,
            ))
        }
    }
}

impl Default for UserServiceHealthCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector for UserServiceHealthCollector {
    type Error = String;

    fn collect(&mut self) -> Result<Option<Event>, Self::Error> {
        let now = Instant::now();
        if now < self.next_poll {
            return Ok(None);
        }
        self.next_poll = now + POLL_INTERVAL;

        let output = Command::new("systemctl")
            .args(["--user", "--failed", "--no-legend", "--plain", "--no-pager"])
            .output()
            .map_err(|error| format!("failed to run systemctl --user --failed: {error}"))?;

        // systemctl commonly returns success for an empty or non-empty --failed
        // listing. Treat other failures as collector failures rather than
        // pretending the host is healthy.
        if !output.status.success() {
            return Err(format!(
                "systemctl --user --failed exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("systemctl output was not UTF-8: {error}"))?;
        let failed = parse_failed_units(&stdout);

        Ok(self.observe_failed_units(failed))
    }
}

#[must_use]
pub fn observation_for_event(event: &Event) -> InitiativeObservation {
    let failed_count = match event.payload.get("failed_count") {
        Some(EventValue::Unsigned(value)) => *value,
        _ => 0,
    };
    let failed_units = event
        .payload
        .get("failed_units")
        .and_then(event_text)
        .unwrap_or("none");
    let newly_failed = event
        .payload
        .get("newly_failed")
        .and_then(event_text)
        .unwrap_or("none");
    let recovered = event
        .payload
        .get("recovered")
        .and_then(event_text)
        .unwrap_or("none");

    let summary = match event.kind.as_str() {
        "system.user_services.recovered" => {
            format!("User service health recovered. Recovered units: {recovered}.")
        }
        _ => format!(
            "Failed user services changed. Failed count: {failed_count}. Current failed units: {failed_units}. Newly failed: {newly_failed}. Recovered: {recovered}."
        ),
    };

    InitiativeObservation {
        source: event.source.as_str().to_string(),
        kind: event.kind.as_str().to_string(),
        summary,
    }
}

fn event_text(value: &EventValue) -> Option<&str> {
    match value {
        EventValue::Text(value) => Some(value.as_str()),
        _ => None,
    }
}

fn parse_failed_units(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            let mut fields = line.split_whitespace();
            let first = fields.next()?;
            let unit = if first == "●" {
                fields.next()?
            } else {
                first
            };

            valid_unit_name(unit).then(|| unit.to_string())
        })
        .collect()
}

fn valid_unit_name(unit: &str) -> bool {
    !unit.is_empty()
        && unit.len() <= 256
        && unit
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@_.:-".contains(character))
}

fn build_health_event(
    kind: &str,
    severity: Severity,
    failed: &BTreeSet<String>,
    newly_failed: &BTreeSet<String>,
    recovered: &BTreeSet<String>,
) -> Event {
    Event {
        id: EventId::new(format!(
            "omarchy-user-health-{}-{}",
            std::process::id(),
            unique_nanos()
        )),
        occurred_at: EventTimestamp::from_unix_millis(unix_millis()),
        source: EventSource::new("omarchy.systemd.user"),
        kind: EventKind::new(kind),
        severity,
        sensitivity: Sensitivity::Standard,
        correlation_id: None,
        payload: EventPayload::new()
            .with_field(
                "failed_count",
                EventValue::Unsigned(u64::try_from(failed.len()).unwrap_or(u64::MAX)),
            )
            .with_field("failed_units", EventValue::Text(join_units(failed)))
            .with_field("newly_failed", EventValue::Text(join_units(newly_failed)))
            .with_field("recovered", EventValue::Text(join_units(recovered))),
    }
}

fn join_units(units: &BTreeSet<String>) -> String {
    if units.is_empty() {
        "none".into()
    } else {
        units.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(units: &[&str]) -> BTreeSet<String> {
        units.iter().map(|unit| (*unit).to_string()).collect()
    }

    #[test]
    fn parser_extracts_plain_and_bullet_prefixed_units() {
        let parsed = parse_failed_units(
            "example.service loaded failed failed Example Service\n● other@one.service loaded failed failed Other\n",
        );

        assert_eq!(parsed, set(&["example.service", "other@one.service"]));
    }

    #[test]
    fn healthy_initial_baseline_emits_nothing() {
        let mut collector = UserServiceHealthCollector::new();
        assert!(collector.observe_failed_units(BTreeSet::new()).is_none());
    }

    #[test]
    fn existing_startup_failure_emits_one_current_condition_event() {
        let mut collector = UserServiceHealthCollector::new();
        let event = collector
            .observe_failed_units(set(&["broken.service"]))
            .expect("existing failure should produce an event");

        assert_eq!(event.kind.as_str(), "system.user_services.failed");
        assert_eq!(event.severity, Severity::Error);
        assert_eq!(
            event.payload.get("failed_count"),
            Some(&EventValue::Unsigned(1))
        );

        assert!(
            collector
                .observe_failed_units(set(&["broken.service"]))
                .is_none()
        );
    }

    #[test]
    fn changed_failure_set_reports_new_and_recovered_units() {
        let mut collector = UserServiceHealthCollector::new();
        let _ = collector.observe_failed_units(set(&["one.service", "old.service"]));

        let event = collector
            .observe_failed_units(set(&["one.service", "new.service"]))
            .expect("changed set should produce event");

        assert_eq!(event.kind.as_str(), "system.user_services.failed_changed");
        assert_eq!(
            event.payload.get("newly_failed"),
            Some(&EventValue::Text("new.service".into()))
        );
        assert_eq!(
            event.payload.get("recovered"),
            Some(&EventValue::Text("old.service".into()))
        );
    }

    #[test]
    fn complete_recovery_emits_info_event_and_useful_observation() {
        let mut collector = UserServiceHealthCollector::new();
        let _ = collector.observe_failed_units(set(&["broken.service"]));

        let event = collector
            .observe_failed_units(BTreeSet::new())
            .expect("recovery should produce event");
        let observation = observation_for_event(&event);

        assert_eq!(event.kind.as_str(), "system.user_services.recovered");
        assert_eq!(event.severity, Severity::Info);
        assert!(observation.summary.contains("broken.service"));
    }
}
