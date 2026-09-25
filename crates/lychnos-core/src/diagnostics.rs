//! Ordinary diagnostic logging boundaries for Lychnos.
//!
//! Diagnostic records are for development and troubleshooting. They are
//! deliberately separate from the security audit trail and do not carry the
//! same retention, integrity, or mandatory-recording guarantees.

use std::convert::Infallible;

/// Severity of one ordinary diagnostic record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

/// One ordinary diagnostic record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRecord {
    pub level: DiagnosticLevel,
    pub component: String,
    pub message: String,
}

impl DiagnosticRecord {
    /// Creates one diagnostic record.
    #[must_use]
    pub fn new(
        level: DiagnosticLevel,
        component: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            level,
            component: component.into(),
            message: message.into(),
        }
    }
}

/// Destination for ordinary diagnostic records.
pub trait DiagnosticSink {
    type Error;

    /// Emits one diagnostic record.
    fn emit(&mut self, record: DiagnosticRecord) -> Result<(), Self::Error>;
}

/// In-memory diagnostic sink used during the foundation phase.
#[derive(Debug, Default)]
pub struct InMemoryDiagnosticLog {
    records: Vec<DiagnosticRecord>,
}

impl InMemoryDiagnosticLog {
    /// Creates an empty diagnostic log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Returns diagnostic records in insertion order.
    #[must_use]
    pub fn records(&self) -> &[DiagnosticRecord] {
        &self.records
    }

    /// Returns the number of diagnostic records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether no diagnostic records exist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl DiagnosticSink for InMemoryDiagnosticLog {
    type Error = Infallible;

    fn emit(&mut self, record: DiagnosticRecord) -> Result<(), Self::Error> {
        self.records.push(record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_record_preserves_troubleshooting_context() {
        let record = DiagnosticRecord::new(
            DiagnosticLevel::Warning,
            "collector",
            "Mock collector returned no event",
        );

        assert_eq!(record.level, DiagnosticLevel::Warning);
        assert_eq!(record.component, "collector");
        assert_eq!(record.message, "Mock collector returned no event");
    }

    #[test]
    fn in_memory_diagnostic_log_records_in_order() {
        let mut log = InMemoryDiagnosticLog::new();

        log.emit(DiagnosticRecord::new(
            DiagnosticLevel::Debug,
            "runtime",
            "first",
        ))
        .expect("in-memory diagnostic emit cannot fail");

        log.emit(DiagnosticRecord::new(
            DiagnosticLevel::Info,
            "runtime",
            "second",
        ))
        .expect("in-memory diagnostic emit cannot fail");

        assert_eq!(log.len(), 2);
        assert_eq!(log.records()[0].message, "first");
        assert_eq!(log.records()[1].message, "second");
    }

    #[test]
    fn new_diagnostic_log_is_empty() {
        let log = InMemoryDiagnosticLog::new();

        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }
}
