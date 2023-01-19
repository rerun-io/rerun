use crate::Event;

// All of our telemetry events are defined in this one file, to facilitate auditing.

// TODO(cmc): all rerun events need hashed application_id + recording_id
// TODO(cmc): add first rerun events

// ---

impl Event {
    pub fn update_metadata() -> Self {
        let rerun_version = env!("CARGO_PKG_VERSION").to_owned();
        let rust_version = env!("CARGO_PKG_RUST_VERSION").to_owned();
        let target = include!(concat!(env!("OUT_DIR"), "/target.rs")).to_owned();
        Self::update("update_metadata".into())
            .with_prop("rerun_version".into(), rerun_version)
            .with_prop("rust_version".into(), rust_version)
            .with_prop("target".into(), target)
    }

    pub fn viewer_opened() -> Self {
        Self::append("viewer_opened".into())
    }
}
