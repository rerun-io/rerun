use std::sync::Arc;

use futures::future::BoxFuture;
use serde_json::{Map, Value};

/// Describes one tool to the model.
///
/// A tool is a function that the model can ask the host to run.
/// For example, the Rerun Viewer offers `get_state` to read what is loaded and shown,
/// `set_time_cursor` to move the time cursor, and `set_blueprint` to change the layout of views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDefinition {
    /// What the model calls the tool by. Unique within a toolbox.
    pub name: String,

    /// What the tool does, written for the model.
    pub description: String,

    /// JSON Schema of the arguments the tool takes.
    pub input_schema: Map<String, Value>,
}

/// One piece of what a tool call returned, shown to the model and in the transcript.
#[derive(Clone, Debug)]
pub enum ToolOutput {
    Text(String),

    Image {
        /// The encoded image, e.g. a PNG.
        bytes: Arc<[u8]>,

        /// The MIME type of `bytes`, e.g. `image/png`.
        mime_type: String,
    },
}

/// The tools a host offers a model-API agent.
///
/// Subprocess agents get their tools elsewhere, e.g. from MCP servers.
pub trait AgentToolbox: Send + Sync {
    /// Every tool on offer, sent to the model with each request.
    fn definitions(&self) -> Vec<ToolDefinition>;

    /// Whether the user is asked before each call of the tool `name`.
    ///
    /// Tools that only read should return `false`, so that the agent can look around freely.
    fn requires_approval(&self, name: &str) -> bool;

    /// Runs the tool `name`.
    ///
    /// An `Err` is reported to the model as a failed call, with its message, so it can correct
    /// itself.
    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'static, Result<Vec<ToolOutput>, Box<dyn std::error::Error + Send + Sync>>>;
}
