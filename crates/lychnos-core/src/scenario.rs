//! Deterministic multi-step simulation scenarios for the Lychnos prototype.
//!
//! Scenario execution is machine-independent and performs no real operating-
//! system actions. It composes existing runtime and collector boundaries so
//! Phase 2 can exercise stateful behavior across multiple steps.

use crate::{
    collector::Collector,
    orchestrator::{CollectorCycleError, FoundationCycle, FoundationRuntime},
    providers::{IdProvider, TimeProvider},
    runtime::RuntimeTransition,
};

/// One deterministic operation in a prototype simulation scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioStep {
    /// Poll the supplied collector once.
    Collect,

    /// Request Game Mode.
    EnterGameMode,

    /// Disable Lychnos.
    Disable,

    /// Explicitly return to Normal mode.
    EnableNormal,
}

/// Result produced by one deterministic scenario step.
#[derive(Debug, Clone, PartialEq)]
pub enum ScenarioStepResult<E> {
    /// Result of one collector poll and foundation-runtime cycle.
    Collection(Result<Option<Box<FoundationCycle>>, CollectorCycleError<E>>),

    /// Result of one requested runtime-mode transition.
    RuntimeTransition(RuntimeTransition),
}

/// Runs an ordered simulation scenario against one existing runtime and
/// collector.
///
/// Collector failures are captured as individual step results rather than
/// aborting the entire scenario. This allows later steps to prove recovery
/// behavior deterministically.
pub fn run_scenario<I, T, C>(
    runtime: &mut FoundationRuntime<I, T>,
    collector: &mut C,
    steps: impl IntoIterator<Item = ScenarioStep>,
) -> Vec<ScenarioStepResult<C::Error>>
where
    I: IdProvider,
    T: TimeProvider,
    C: Collector,
{
    steps
        .into_iter()
        .map(|step| match step {
            ScenarioStep::Collect => ScenarioStepResult::Collection(
                runtime
                    .collect_once(collector)
                    .map(|cycle| cycle.map(Box::new)),
            ),
            ScenarioStep::EnterGameMode => {
                ScenarioStepResult::RuntimeTransition(runtime.enter_game_mode())
            }
            ScenarioStep::Disable => ScenarioStepResult::RuntimeTransition(runtime.disable()),
            ScenarioStep::EnableNormal => {
                ScenarioStepResult::RuntimeTransition(runtime.enable_normal())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audit::AuditEventKind,
        collector::{InjectedCollectorError, ScriptedCollector, ScriptedCollectorStep},
        event::{
            Event, EventId, EventKind, EventPayload, EventSource, EventTimestamp, Sensitivity,
            Severity,
        },
        executor::MockExecutionOutcome,
        providers::{FixedTimeProvider, SequenceIdProvider},
        runtime::RuntimeMode,
    };

    fn runtime() -> FoundationRuntime<SequenceIdProvider, FixedTimeProvider> {
        FoundationRuntime::new(
            RuntimeMode::Normal,
            SequenceIdProvider::default(),
            FixedTimeProvider::new(1_800_000_000_123),
        )
    }

    fn event(id: &str, kind: &str) -> Event {
        Event {
            id: EventId::new(id),
            occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_123),
            source: EventSource::new("scenario"),
            kind: EventKind::new(kind),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        }
    }

    #[test]
    fn multi_event_scenario_preserves_event_order() {
        let mut runtime = runtime();

        let mut collector = ScriptedCollector::from_steps([
            ScriptedCollectorStep::Event(event("scenario-event-1", "scenario.first")),
            ScriptedCollectorStep::Event(event("scenario-event-2", "scenario.second")),
        ]);

        let results = run_scenario(
            &mut runtime,
            &mut collector,
            [ScenarioStep::Collect, ScenarioStep::Collect],
        );

        assert_eq!(results.len(), 2);

        let ScenarioStepResult::Collection(Ok(Some(first))) = &results[0] else {
            panic!("first scenario step should process an event");
        };

        let ScenarioStepResult::Collection(Ok(Some(second))) = &results[1] else {
            panic!("second scenario step should process an event");
        };

        assert_eq!(first.event.id.as_str(), "scenario-event-1");
        assert_eq!(second.event.id.as_str(), "scenario-event-2");

        assert!(matches!(
            first.outcome,
            MockExecutionOutcome::WouldExecute { .. }
        ));
        assert!(matches!(
            second.outcome,
            MockExecutionOutcome::WouldExecute { .. }
        ));

        assert!(collector.is_empty());
    }

    #[test]
    fn game_mode_pauses_scenario_collection_without_consuming_event() {
        let mut runtime = runtime();

        let mut collector = ScriptedCollector::from_steps([ScriptedCollectorStep::Event(event(
            "event-after-game-mode",
            "scenario.resumed",
        ))]);

        let results = run_scenario(
            &mut runtime,
            &mut collector,
            [
                ScenarioStep::EnterGameMode,
                ScenarioStep::Collect,
                ScenarioStep::EnableNormal,
                ScenarioStep::Collect,
            ],
        );

        let ScenarioStepResult::RuntimeTransition(entered) = results[0] else {
            panic!("first step should enter Game Mode");
        };

        assert_eq!(entered.from, RuntimeMode::Normal);
        assert_eq!(entered.to, RuntimeMode::GameMode);
        assert!(entered.changed);

        assert!(matches!(
            &results[1],
            ScenarioStepResult::Collection(Ok(None))
        ));

        let ScenarioStepResult::RuntimeTransition(enabled) = results[2] else {
            panic!("third step should return to Normal");
        };

        assert_eq!(enabled.from, RuntimeMode::GameMode);
        assert_eq!(enabled.to, RuntimeMode::Normal);
        assert!(enabled.changed);

        let ScenarioStepResult::Collection(Ok(Some(cycle))) = &results[3] else {
            panic!("queued event should resume after leaving Game Mode");
        };

        assert_eq!(cycle.event.id.as_str(), "event-after-game-mode");
        assert!(collector.is_empty());
        assert_eq!(runtime.mode(), RuntimeMode::Normal);
    }

    #[test]
    fn scenario_continues_after_injected_collector_failure() {
        let mut runtime = runtime();

        let injected = InjectedCollectorError::new("scenario failure");

        let mut collector = ScriptedCollector::from_steps([
            ScriptedCollectorStep::Event(event("event-before-failure", "scenario.before")),
            ScriptedCollectorStep::Error(injected.clone()),
            ScriptedCollectorStep::Event(event("event-after-failure", "scenario.after")),
        ]);

        let results = run_scenario(
            &mut runtime,
            &mut collector,
            [
                ScenarioStep::Collect,
                ScenarioStep::Collect,
                ScenarioStep::Collect,
            ],
        );

        assert!(matches!(
            &results[0],
            ScenarioStepResult::Collection(Ok(Some(_)))
        ));

        assert_eq!(
            results[1],
            ScenarioStepResult::Collection(Err(CollectorCycleError::Collector(injected)))
        );

        let ScenarioStepResult::Collection(Ok(Some(recovered))) = &results[2] else {
            panic!("scenario should continue after collector failure");
        };

        assert_eq!(recovered.event.id.as_str(), "event-after-failure");
        assert!(collector.is_empty());

        assert!(runtime.diagnostics_log().records().iter().any(|record| {
            record.component == "collector" && record.message == "Collector returned an error"
        }));
    }

    #[test]
    fn disabled_state_carries_across_scenario_steps_until_explicit_enable() {
        let mut runtime = runtime();

        let mut collector = ScriptedCollector::from_steps([ScriptedCollectorStep::Event(event(
            "event-after-enable",
            "scenario.enabled",
        ))]);

        let results = run_scenario(
            &mut runtime,
            &mut collector,
            [
                ScenarioStep::Disable,
                ScenarioStep::EnterGameMode,
                ScenarioStep::Collect,
                ScenarioStep::EnableNormal,
                ScenarioStep::Collect,
            ],
        );

        let ScenarioStepResult::RuntimeTransition(disabled) = results[0] else {
            panic!("first step should disable runtime");
        };

        assert_eq!(disabled.to, RuntimeMode::Disabled);
        assert!(disabled.changed);

        let ScenarioStepResult::RuntimeTransition(game_request) = results[1] else {
            panic!("second step should be a mode transition request");
        };

        assert_eq!(game_request.from, RuntimeMode::Disabled);
        assert_eq!(game_request.to, RuntimeMode::Disabled);
        assert!(!game_request.changed);

        assert!(matches!(
            &results[2],
            ScenarioStepResult::Collection(Ok(None))
        ));

        let ScenarioStepResult::RuntimeTransition(enabled) = results[3] else {
            panic!("fourth step should explicitly enable runtime");
        };

        assert_eq!(enabled.from, RuntimeMode::Disabled);
        assert_eq!(enabled.to, RuntimeMode::Normal);
        assert!(enabled.changed);

        let ScenarioStepResult::Collection(Ok(Some(cycle))) = &results[4] else {
            panic!("event should process after explicit re-enable");
        };

        assert_eq!(cycle.event.id.as_str(), "event-after-enable");
        assert!(collector.is_empty());

        let transition_records = runtime
            .audit_log()
            .records()
            .iter()
            .filter(|record| record.kind == AuditEventKind::RuntimeModeChanged)
            .count();

        assert_eq!(transition_records, 3);
    }
}
