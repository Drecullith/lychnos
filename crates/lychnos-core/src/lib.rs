//! Platform-independent foundations for Project Lychnos.

pub mod action;
pub mod analyzer;
pub mod approval;
pub mod audit;
pub mod bus;
pub mod collector;
pub mod config;
pub mod diagnostics;
pub mod event;
pub mod executor;
pub mod interaction;
pub mod memory;
pub mod orchestrator;
pub mod permission;
pub mod persona;
pub mod presentation;
pub mod providers;
pub mod resource;
pub mod runtime;
pub mod scenario;

/// Canonical project name.
pub const PROJECT_NAME: &str = "Lychnos";

/// Returns the version of the Lychnos core crate.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_identity_is_available() {
        assert_eq!(PROJECT_NAME, "Lychnos");
        assert_eq!(version(), "0.1.0");
    }
}
