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

mod run;

pub use run::{run, CallSource};

// NOTE: Have a look at `re_sdk/src/lib.rs` for an accurate listing of all these symbols.
#[cfg(feature = "sdk")]
pub use re_sdk::*;
