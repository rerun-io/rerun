//! All of our telemetry events are defined in this one file, to facilitate auditing.

use crate::{Event, Property};

// ---

impl Event {
    /// Creates an `Event::Update` with basic system information: Rerun version, Rust version,
    /// target platform triplet...
    pub fn update_metadata(props: impl IntoIterator<Item = (String, Property)>) -> Self {
        let rerun_version = env!("CARGO_PKG_VERSION").to_owned();
        let rust_version = env!("CARGO_PKG_RUST_VERSION").to_owned();
        let target_triple = env!("__RERUN_TARGET_TRIPLE").to_owned();
        let git_hash = env!("__RERUN_GIT_HASH").to_owned();

        let mut event = Self::update("update_metadata".into())
            .with_prop("rerun_version".into(), rerun_version)
            .with_prop("rust_version".into(), rust_version)
            .with_prop("target".into(), target_triple)
            .with_prop("git_hash".into(), git_hash);

        for (name, value) in props {
            event = event.with_prop(name.into(), value);
        }

        event
    }
}
