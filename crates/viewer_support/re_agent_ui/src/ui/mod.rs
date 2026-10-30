mod agent_panel;
mod chat_ui;
mod conversation;
mod conversation_list;
mod elicitation_ui;
mod linkify;
mod pasted_image;
mod setup_ui;
mod tool_call_ui;
mod transcript_ui;

pub use agent_panel::{AgentPanel, RECOMMENDED_WIDTH};

/// Scrollable request contents leave room for action buttons and the chat composer.
const MAX_REQUEST_CONTENT_HEIGHT: f32 = 320.0;

/// Which of the two views the panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Chat,
}
