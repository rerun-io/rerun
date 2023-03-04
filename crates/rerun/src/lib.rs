//! # Rerun - log point clouds, images, etc and visualize them effortlessly
//!
//! Add the `rerun` library to your crate with `cargo add rerun`.
//!
//! There is also a `rerun` binary.
//! The binary is required in order to stream log data
//! over the networks, and to open our `.rrd` data files.
//! If you need it, install the `rerun` binary with `cargo install rerun`.
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
//! ## Using the `rerun` library
//! #### Logging
//!
//! ```
//! # use rerun::external::image;
//! # fn capture_image() -> image::DynamicImage { Default::default() }
//! # fn positions() -> Vec<rerun::components::Point3D> { Default::default() }
//! # fn colors() -> Vec<rerun::components::ColorRGBA> { Default::default() }
//! let mut rr_session = rerun::Session::init("my_app", true);
//!
//! let points: Vec<rerun::components::Point3D> = positions();
//! let colors: Vec<rerun::components::ColorRGBA> = colors();
//! let image: image::DynamicImage = capture_image();
//!
//! rerun::MsgSender::new("points")
//!     .with_component(&points)?
//!     .with_component(&colors)?
//!     .send(&mut rr_session)?;
//!
//! rerun::MsgSender::new("image")
//!     .with_component(&[rerun::components::Tensor::from_image(image)?])?
//!     .send(&mut rr_session)?;
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! See [`Session`] and [`MsgSender`] for details.
//!
//! #### Streaming
//! To stream log data to an awaiting `rerun` process, you can do this:
//! Start `rerun` in a terminal by just running `rerun`.
//!
//! Then do this:
//!
//! ``` no_run
//! let mut rr_session = rerun::Session::init("my_app", true);
//! rr_session.connect(rerun::default_server_addr());
//! ```
//!
//! #### Buffering
//!
//! ``` no_run
//! # fn log_using(rr_session: &mut rerun::Session) {}
//!
//! let mut rr_session = rerun::Session::init("my_app", true);
//! log_using(&mut rr_session);
//! rr_session.show();
//! ```
//!
//! ## Binary
//! The `rerun` binary is required in order to stream log data
//! over the networks, and to open our `.rrd` data files.
//!
//! The binary can act either as a server, a viewer, or both,
//! depending on which options you use when you start it.
//!
//! ```ignore
//! cargo install rerun
//! rerun --help
//! ```
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![warn(missing_docs)] // Let's keep the this crate well-documented!

mod crash_handler;
mod run;

/// Module for integrating with the [`clap`](https://crates.io/crates/clap) command line argument parser.
#[cfg(all(feature = "sdk", not(target_arch = "wasm32")))]
pub mod clap;

/// Methods for spawning the native viewer and streaming the SDK log stream to it.
#[cfg(all(feature = "sdk", feature = "native_viewer"))]
pub mod native_viewer;

#[cfg(all(feature = "sdk", feature = "web_viewer"))]
mod web_viewer;

pub use run::{run, CallSource};

// NOTE: Have a look at `re_sdk/src/lib.rs` for an accurate listing of all these symbols.
#[cfg(feature = "sdk")]
pub use re_sdk::*;

#[cfg(all(feature = "sdk", feature = "web_viewer"))]
pub use web_viewer::WebViewerSessionExt;

/// Re-exports of other crates.
pub mod external {
    #[cfg(feature = "native_viewer")]
    pub use re_viewer;

    #[cfg(feature = "sdk")]
    pub use re_sdk::external::*;
}
