mod agent_panel;
mod chat_ui;
mod linkify;
mod setup_ui;
mod tool_call_ui;
mod transcript_ui;

pub use agent_panel::AgentPanel;

/// Which of the two views the panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Chat,
}
