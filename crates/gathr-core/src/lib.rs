//! Gathr's core. A placeholder so the workspace and CI have something real to
//! build and test; the architecture in `docs/` decides what lives here.

/// The version of this build, from Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_cargo() {
        assert_eq!(version(), "0.0.0");
    }
}
pub fn   badly_formatted( ) {}
