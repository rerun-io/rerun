//! Chat UI for driving a coding agent over the Agent Client Protocol (ACP).
//!
//! The protocol, session state, and transcript live in [`re_agent`]; this crate renders them
//! with egui and `re_ui`.
//!
//! This crate is a generic chat UI that any egui app could use, and must not know about the
//! Rerun Viewer: what ties the agent to the viewer goes in `re_agent_panel`.
//! `scripts/check_crate_layers.py` enforces this.

mod ui;

pub use ui::{AgentPanel, RECOMMENDED_WIDTH};
