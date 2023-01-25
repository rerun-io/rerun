//! All of our telemetry events are defined in this one file, to facilitate auditing.

use crate::Event;

// ---

impl Event {
    /// Creates an `Event::Update` with basic system information: Rerun version, Rust version,
    /// target platform triplet...
    pub fn update_metadata() -> Self {
        let rerun_version = env!("CARGO_PKG_VERSION").to_owned();
        let rust_version = env!("CARGO_PKG_RUST_VERSION").to_owned();
        let target_triple = env!("__RERUN_TARGET_TRIPLE").to_owned();
        let git_hash = env!("__RERUN_GIT_HASH").to_owned();
        Self::update("update_metadata".into())
            .with_prop("rerun_version".into(), rerun_version)
            .with_prop("rust_version".into(), rust_version)
            .with_prop("target".into(), target_triple)
            .with_prop("git_hash".into(), git_hash)
    }

    pub fn viewer_started(kind: &str) -> Self {
        Self::append("viewer_started".into()).with_prop("kind".into(), kind.to_owned())
    }

    // TODO: document exactly what gets fired when...

    // TODO:
    //
    // for `viewer_started`:
    //   - kind = [
    //       # viewer started as a native app, whether that's a `show`, a `spawn_and_connect`,
    //       # a `cargo run`...
    //       "native",
    //       # means that a web client connected to the websocket server.
    //       "web",
    //   ]
    //
    // for `data_source_opened`:
    //   - source_type = [
    //       # loading an rrd file from.. somewhere
    //       "rrd",
    //       # getting fed a real-time stream from a local TCP connection (`spawn_and_connect`)
    //       "network_local",
    //       # getting fed a real-time stream from a remote TCP connection (`connect`)
    //       "network_remote",
    //       # getting fed a "buffered real-time" stream from memory (`show`)
    //       "buffered"
    //   ]
    //
    // `cargo r --features web -- --web-viewer examples/out/avocado.rrd`:
    // - viewer started on client connection (kind=web)
    pub fn data_source_opened(kind: &str) -> Self {
        Self::append("data_source_opened".into()).with_prop("kind".into(), kind.to_owned())
    }
}
