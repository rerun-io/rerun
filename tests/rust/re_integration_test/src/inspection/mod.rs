//! Harness that can test against a real app instance using `egui_inspection`.
//!
//! The entry point is [`InspectionHarness::spawn`], which drives one of the viewers described by
//! [`TargetViewer`], selected via the environment ([`TestEnv`]).

mod config;
mod connection;
mod env;
mod harness;

pub use config::HarnessConfig;
pub use env::{TargetViewer, TestEnv};
pub use harness::InspectionHarness;
