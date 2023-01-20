//! All of our telemetry events are defined in this one file, to facilitate auditing.

use crate::Event;

// ---

impl Event {
    /// Creates an `Event::Update` with basic system information: Rerun version, Rust version,
    /// target platform triplet...
    pub fn update_metadata() -> Self {
        let rerun_version = env!("CARGO_PKG_VERSION").to_owned();
        let rust_version = env!("CARGO_PKG_RUST_VERSION").to_owned();
        let target = include!(concat!(env!("OUT_DIR"), "/target.rs")).to_owned();
        Self::update("update_metadata".into())
            .with_prop("rerun_version".into(), rerun_version)
            .with_prop("rust_version".into(), rust_version)
            .with_prop("target".into(), target)
    }
}
