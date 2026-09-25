//! Machine-independent resource instrumentation for Lychnos.
//!
//! Phase 2 records typed observations and evaluates caller-supplied budgets.
//! It deliberately does not choose an operating-system sampler, polling
//! interval, background thread, or async runtime.

use std::convert::Infallible;

use crate::runtime::RuntimeMode;

/// Stable name for one measured resource quantity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceMetric(String);

impl ResourceMetric {
    /// Creates a resource metric name.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the metric name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Unit attached to a resource observation and budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceUnit {
    /// Raw bytes.
    Bytes,
    /// Microseconds of measured activity.
    Microseconds,
    /// Dimensionless event or operation count.
    Count,
    /// Hundredths of one percentage point; 10_000 equals 100%.
    BasisPoints,
}

impl ResourceUnit {
    /// Stable machine-readable unit label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Microseconds => "microseconds",
            Self::Count => "count",
            Self::BasisPoints => "basis_points",
        }
    }
}
/// One caller-supplied resource measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceObservation {
    pub metric: ResourceMetric,
    pub unit: ResourceUnit,
    pub value: u64,
    pub runtime_mode: RuntimeMode,
    pub component: String,
}

impl ResourceObservation {
    /// Creates one typed observation.
    #[must_use]
    pub fn new(
        metric: ResourceMetric,
        unit: ResourceUnit,
        value: u64,
        runtime_mode: RuntimeMode,
        component: impl Into<String>,
    ) -> Self {
        Self {
            metric,
            unit,
            value,
            runtime_mode,
            component: component.into(),
        }
    }
}
/// Maximum accepted value for one metric in one runtime mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBudget {
    pub metric: ResourceMetric,
    pub unit: ResourceUnit,
    pub runtime_mode: RuntimeMode,
    pub max_value: u64,
}

impl ResourceBudget {
    /// Creates one mode-specific budget.
    #[must_use]
    pub const fn new(
        metric: ResourceMetric,
        unit: ResourceUnit,
        runtime_mode: RuntimeMode,
        max_value: u64,
    ) -> Self {
        Self {
            metric,
            unit,
            runtime_mode,
            max_value,
        }
    }
    /// Evaluates one observation against this exact metric/unit/mode budget.
    pub fn evaluate(
        &self,
        observation: &ResourceObservation,
    ) -> Result<ResourceBudgetEvaluation, ResourceBudgetMismatch> {
        if observation.metric != self.metric {
            return Err(ResourceBudgetMismatch::Metric);
        }
        if observation.unit != self.unit {
            return Err(ResourceBudgetMismatch::Unit);
        }
        if observation.runtime_mode != self.runtime_mode {
            return Err(ResourceBudgetMismatch::RuntimeMode);
        }

        if observation.value <= self.max_value {
            Ok(ResourceBudgetEvaluation::Within {
                headroom: self.max_value - observation.value,
            })
        } else {
            Ok(ResourceBudgetEvaluation::Exceeded {
                excess: observation.value - self.max_value,
            })
        }
    }
}
/// Why an observation cannot be evaluated by one budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceBudgetMismatch {
    Metric,
    Unit,
    RuntimeMode,
}

/// Result of comparing one compatible observation with its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceBudgetEvaluation {
    Within { headroom: u64 },
    Exceeded { excess: u64 },
}

impl ResourceBudgetEvaluation {
    /// Returns whether the observation exceeded its budget.
    #[must_use]
    pub const fn exceeded(self) -> bool {
        matches!(self, Self::Exceeded { .. })
    }
}

/// Destination for resource observations.
pub trait ResourceSink {
    type Error;

    fn record(&mut self, observation: ResourceObservation) -> Result<(), Self::Error>;
}
/// In-memory resource observation log used by deterministic simulations.
#[derive(Debug, Default)]
pub struct InMemoryResourceLog {
    observations: Vec<ResourceObservation>,
}

impl InMemoryResourceLog {
    /// Creates an empty observation log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            observations: Vec::new(),
        }
    }

    /// Returns observations in insertion order.
    #[must_use]
    pub fn observations(&self) -> &[ResourceObservation] {
        &self.observations
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }
}
impl ResourceSink for InMemoryResourceLog {
    type Error = Infallible;

    fn record(&mut self, observation: ResourceObservation) -> Result<(), Self::Error> {
        self.observations.push(observation);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_observation(mode: RuntimeMode, value: u64) -> ResourceObservation {
        ResourceObservation::new(
            ResourceMetric::new("process.cpu.utilization"),
            ResourceUnit::BasisPoints,
            value,
            mode,
            "foundation-runtime",
        )
    }

    #[test]
    fn observation_preserves_typed_context() {
        let observation = cpu_observation(RuntimeMode::GameMode, 125);
        assert_eq!(observation.metric.as_str(), "process.cpu.utilization");
        assert_eq!(observation.unit, ResourceUnit::BasisPoints);
        assert_eq!(observation.value, 125);
        assert_eq!(observation.runtime_mode, RuntimeMode::GameMode);
        assert_eq!(observation.component, "foundation-runtime");
    }

    #[test]
    fn mode_specific_budget_reports_headroom_and_excess() {
        let budget = ResourceBudget::new(
            ResourceMetric::new("process.cpu.utilization"),
            ResourceUnit::BasisPoints,
            RuntimeMode::GameMode,
            200,
        );

        assert_eq!(
            budget.evaluate(&cpu_observation(RuntimeMode::GameMode, 125)),
            Ok(ResourceBudgetEvaluation::Within { headroom: 75 })
        );
        assert_eq!(
            budget.evaluate(&cpu_observation(RuntimeMode::GameMode, 275)),
            Ok(ResourceBudgetEvaluation::Exceeded { excess: 75 })
        );
    }
    #[test]
    fn budget_rejects_metric_unit_and_mode_mismatches() {
        let budget = ResourceBudget::new(
            ResourceMetric::new("process.cpu.utilization"),
            ResourceUnit::BasisPoints,
            RuntimeMode::GameMode,
            200,
        );

        let wrong_metric = ResourceObservation::new(
            ResourceMetric::new("process.memory.rss"),
            ResourceUnit::BasisPoints,
            100,
            RuntimeMode::GameMode,
            "runtime",
        );
        assert_eq!(
            budget.evaluate(&wrong_metric),
            Err(ResourceBudgetMismatch::Metric)
        );

        let wrong_unit = ResourceObservation::new(
            ResourceMetric::new("process.cpu.utilization"),
            ResourceUnit::Bytes,
            100,
            RuntimeMode::GameMode,
            "runtime",
        );
        assert_eq!(
            budget.evaluate(&wrong_unit),
            Err(ResourceBudgetMismatch::Unit)
        );

        assert_eq!(
            budget.evaluate(&cpu_observation(RuntimeMode::Normal, 100)),
            Err(ResourceBudgetMismatch::RuntimeMode)
        );
    }

    #[test]
    fn in_memory_resource_log_preserves_order() {
        let mut log = InMemoryResourceLog::new();
        assert!(log.is_empty());

        log.record(cpu_observation(RuntimeMode::Normal, 400))
            .expect("in-memory resource log cannot fail");
        log.record(cpu_observation(RuntimeMode::GameMode, 100))
            .expect("in-memory resource log cannot fail");

        assert_eq!(log.len(), 2);
        assert_eq!(log.observations()[0].runtime_mode, RuntimeMode::Normal);
        assert_eq!(log.observations()[1].runtime_mode, RuntimeMode::GameMode);
    }

    #[test]
    fn budget_evaluation_exposes_simple_exceeded_flag() {
        assert!(!ResourceBudgetEvaluation::Within { headroom: 1 }.exceeded());
        assert!(ResourceBudgetEvaluation::Exceeded { excess: 1 }.exceeded());
    }
}
