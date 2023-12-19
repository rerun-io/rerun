//! # Rerun - Visualize streams of multimodal data.
//!
//! Add the `rerun` library to your crate with `cargo add rerun`.
//!
//! There is also a `rerun` binary.
//! The binary is required in order to stream log data
//! over the networks, and to open our `.rrd` data files.
//! If you need it, install the `rerun` binary with `cargo install rerun-cli`.
//!
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!
//!
//! ## Links
//! - [Examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust)
//! - [High-level docs](http://rerun.io/docs)
//! - [Rust API docs](https://docs.rs/rerun/)
//! - [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)
//!
//! There are many different ways of sending data to the Rerun Viewer depending on what you're
//! trying to achieve and whether the viewer is running in the same process as your code, in
//! another process, or even as a separate web application.
//!
//! Checkout [SDK Operating Modes](https://www.rerun.io/docs/reference/sdk-operating-modes) for an
//! overview of what's possible and how.
//!
//! If you get stuck on anything, open an issue at <https://github.com/rerun-io/rerun/issues>.
//! You can also ask questions on the [Rerun Discord](https://discord.gg/Gcm8BbTaAj).
//!
//!
//! ## Using the `rerun` binary
//! The `rerun` binary is required in order to stream log data
//! over the networks, and to open our `.rrd` data files.
//!
//! The binary can act either as a server, a viewer, or both,
//! depending on which options you use when you start it.
//!
//! Install it with `cargo install rerun-cli`.
//!
//! Running just `rerun` will start the viewer, waiting for the logging library to connect
//! using [`RecordingStreamBuilder::connect`] (see below).
//!
//! You can run `rerun --help` for more info.
//!
//!
//! ## Using the `rerun` library
//! #### Logging
//! You first create a [`RecordingStream`] using [`RecordingStreamBuilder`].
//! You then use it to log some [`archetypes`] to a given [`EntityPath`] using [`RecordingStream::log`]:
//!
//! ```no_run
//! # use rerun::external::image;
//! # fn capture_image() -> image::DynamicImage { Default::default() }
//! # fn positions() -> Vec<rerun::Position3D> { Default::default() }
//! # fn colors() -> Vec<rerun::Color> { Default::default() }
//! // Stream log data to an awaiting `rerun` process.
//! let rec = rerun::RecordingStreamBuilder::new("rerun_example_app").connect()?;
//!
//! let points: Vec<rerun::Position3D> = positions();
//! let colors: Vec<rerun::Color> = colors();
//! let image: image::DynamicImage = capture_image();
//!
//! rec.set_time_sequence("frame", 42);
//! rec.log("path/to/points", &rerun::Points3D::new(points).with_colors(colors))?;
//! rec.log("path/to/image", &rerun::Image::try_from(image)?)?;
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! #### Streaming to disk
//! Streaming data to a file on disk using the `.rrd` format:
//!
//! ```no_run
//! let rec = rerun::RecordingStreamBuilder::new("rerun_example_app").save("my_data.rrd")?;
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! #### Buffering
//! You can buffer the log messages in memory and then show them in an embedded viewer:
//!
//! ```no_run
//! # fn log_to(rec: &rerun::RecordingStream) {}
//! let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_app").memory()?;
//! log_to(&rec);
//!
//! // Will block program execution!
//! rerun::native_viewer::show(storage.take());
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ```ignore
//! cargo install rerun
//! rerun --help
//! ```
//!
//!
//! ## Forwarding text log events to Rerun
//! See [`Logger`].
//!

#![warn(missing_docs)] // Let's keep the this crate well-documented!

mod run;

#[cfg(feature = "sdk")]
mod sdk;

#[cfg(all(feature = "sdk", not(target_arch = "wasm32")))]
pub mod clap;

/// Methods for spawning the native viewer and streaming the SDK log stream to it.
#[cfg(all(feature = "sdk", feature = "native_viewer"))]
pub mod native_viewer;

#[cfg(feature = "demo")]
pub mod demo_util;

#[cfg(feature = "log")]
pub mod log_integration;

#[cfg(feature = "log")]
pub use re_log::default_log_filter;

#[cfg(feature = "log")]
pub use log_integration::Logger;

pub use run::{run, CallSource};

#[cfg(feature = "sdk")]
pub use sdk::*;

/// Everything needed to build custom `StoreSubscriber`s.
pub use re_data_store::external::re_arrow_store::{
    DataStore, StoreDiff, StoreDiffKind, StoreEvent, StoreGeneration, StoreSubscriber,
};

pub use re_data_source::{
    EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE, EXTERNAL_DATA_LOADER_PREFIX,
};

/// Re-exports of other crates.
pub mod external {
    pub use anyhow;

    pub use re_build_info;
    pub use re_data_source;
    pub use re_data_store;
    pub use re_data_store::external::*;
    pub use re_format;

    #[cfg(all(feature = "sdk", not(target_arch = "wasm32")))]
    pub use clap;

    #[cfg(not(target_arch = "wasm32"))]
    pub use tokio;

    #[cfg(feature = "native_viewer")]
    pub use re_viewer;

    #[cfg(feature = "native_viewer")]
    pub use re_viewer::external::*;

    #[cfg(feature = "sdk")]
    pub use re_sdk::external::*;

    #[cfg(feature = "sdk")]
    pub use re_types;

    #[cfg(feature = "sdk")]
    pub use re_types::external::*;
}
