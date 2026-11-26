//! The interface between an LLM agent and the tools a host offers it: [`AgentToolbox`].
//!
//! A toolbox does not care how it reaches the agent. The viewer's agent panel hands toolboxes
//! to a model-API agent in-process, and `re_rerun_mcp` serves them to coding agents over MCP.
//!
//! The types follow MCP's tool model, so that serving a toolbox over MCP is a direct mapping:
//! [`ToolDefinition`] is MCP's `Tool`, [`ToolEffect`] its tool annotations, [`ToolOutput`] its
//! content blocks, and a [`ToolError`] a result with `isError` set.

mod combined_toolbox;
mod model_preselect;

use std::sync::Arc;

use futures::future::BoxFuture;
use serde_json::{Map, Value};

pub use self::combined_toolbox::CombinedToolbox;
pub use self::model_preselect::{ModelCategory, ModelPreference, ModelPreselect};

/// What the agent may do with a catalog server.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CatalogAccess {
    /// Find and query data, but change nothing.
    ReadOnly,

    /// Also create datasets, rename entries, and upload or register recordings.
    /// Every change still asks the user first.
    #[default]
    ReadWrite,
}

impl CatalogAccess {
    /// Whether the agent may change the catalog.
    pub fn allows_writes(self) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::ReadWrite => true,
        }
    }
}

/// The error a tool call fails with.
///
/// Its message is reported to the model, so it can correct itself.
/// Over MCP, it becomes a tool result with `isError` set, not a protocol error.
pub type ToolError = Box<dyn std::error::Error + Send + Sync>;

/// Describes one tool to the model, like MCP's `Tool`.
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

    /// What running the tool can change, whatever its arguments.
    ///
    /// [`AgentToolbox::requires_approval`] may judge one call differently, from its arguments.
    pub effect: ToolEffect,

    /// Whether the tool may reach beyond the host, e.g. load a URL or send data to a server.
    ///
    /// Over MCP, this is `openWorldHint`.
    pub open_world: bool,
}

/// What running a tool can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolEffect {
    /// Changes nothing.
    ///
    /// Over MCP, this is `readOnlyHint: true`.
    ReadOnly,

    /// Like [`Self::NonDestructive`], and running it again with the same arguments changes
    /// nothing more, e.g. resizing a window.
    ///
    /// Over MCP, this is `idempotentHint: true` and `destructiveHint: false`.
    IdempotentNonDestructive,

    /// Changes only state the user can easily undo, e.g. what the viewer shows.
    ///
    /// Over MCP, this is `destructiveHint: false`.
    NonDestructive,

    /// Anything else.
    ///
    /// Over MCP, this is `destructiveHint: true`.
    Destructive,
}

impl ToolEffect {
    /// Whether the tool changes nothing, which is MCP's `readOnlyHint`.
    pub fn read_only(self) -> bool {
        self == Self::ReadOnly
    }

    /// Whether the tool may change what the user cannot easily undo, which is MCP's
    /// `destructiveHint`.
    pub fn destructive(self) -> bool {
        self == Self::Destructive
    }

    /// Whether running the tool again with the same arguments changes nothing more, which is
    /// MCP's `idempotentHint`.
    pub fn idempotent(self) -> bool {
        matches!(self, Self::ReadOnly | Self::IdempotentNonDestructive)
    }

    /// Whether an agent should ask the user before running the tool.
    pub fn requires_approval(self) -> bool {
        self.destructive()
    }
}

/// One piece of what a tool call returned, shown to the model and in the transcript.
///
/// Each variant is one MCP content block, of type `text` or `image`.
#[derive(Clone, Debug)]
pub enum ToolOutput {
    /// Text for the model, often JSON.
    Text(String),

    Image {
        /// The encoded image, e.g. a PNG.
        bytes: Arc<[u8]>,

        /// The MIME type of `bytes`, e.g. `image/png`.
        mime_type: String,
    },
}

/// The tools a host offers an agent.
///
/// Subprocess agents get their tools elsewhere, e.g. from MCP servers. When one of those serves
/// the same tools, see [`Self::mcp_server_name`].
pub trait AgentToolbox: Send + Sync {
    /// Every tool on offer, sent to the model with each request.
    fn definitions(&self) -> Vec<ToolDefinition>;

    /// Whether the user is asked before this call of the tool `name`, with `arguments`.
    ///
    /// Calls that only read, or that the user can easily undo, should return `false`, so that
    /// the agent can work freely.
    fn requires_approval(&self, name: &str, arguments: &Map<String, Value>) -> bool;

    /// The host-supplied MCP server that serves the same tools, under the same names, to agent
    /// harnesses.
    ///
    /// A harness asks permission for calls to it as `mcp__<server>__<tool>`, and those that
    /// [`Self::requires_approval`] lets through are then approved without asking the user.
    fn mcp_server_name(&self) -> Option<&str> {
        None
    }

    /// Runs the tool `name`.
    ///
    /// An `Err` is reported to the model as a failed call, with its message, so it can correct
    /// itself.
    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'static, Result<Vec<ToolOutput>, ToolError>>;
}
