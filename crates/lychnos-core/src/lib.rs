//! Platform-independent foundations for Project Lychnos.

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
