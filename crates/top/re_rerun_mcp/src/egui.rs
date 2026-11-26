//! The egui toolset: the widget tools of `egui_inspection_tools` (`ui_widget_tree`,
//! `ui_click`, `ui_screenshot`, …), driving the UI of the connected viewer.
//!
//! The tools have no connection of their own: each call looks up the viewer toolset's current
//! one (see [`TransportSource`]).

use std::sync::Arc;
use std::time::Duration;

use egui_inspection_tools::{BoxFuture, Content, Effect, ToolDef, Transport};
use futures::FutureExt as _;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value};

use re_agent_tools::{AgentToolbox, ToolDefinition, ToolEffect, ToolError, ToolOutput};

use crate::toolbox;

/// Operating guidance for the egui tools, part of the MCP `instructions`.
const INSTRUCTIONS: &str = include_str!("egui_instructions.md");

/// Finds the app to drive, once per tool call: `None` while no app is connected.
///
/// Looked up on every call rather than stored, so the egui tools always drive the app the
/// connection's owner is connected to right now.
pub type TransportSource = Arc<dyn Fn() -> Option<Arc<dyn Transport>> + Send + Sync>;

/// The egui widget tools as an [`AgentToolbox`], driving whatever app its transport reaches.
pub struct EguiToolbox {
    transport: TransportSource,
}

impl EguiToolbox {
    /// The tools, driving whatever app `transport` finds at the time of each call.
    pub fn new(transport: TransportSource) -> Self {
        Self { transport }
    }

    /// Whether the agent's tool `name` is one of this toolbox's.
    pub fn owns(name: &str) -> bool {
        egui_inspection_tools::tool(name).is_some()
    }
}

impl AgentToolbox for EguiToolbox {
    fn definitions(&self) -> Vec<ToolDefinition> {
        egui_inspection_tools::tools()
            .iter()
            .map(|def| {
                let ToolDef {
                    name,
                    description,
                    input_schema,
                    effect,
                    title: _,
                    output_schema: _,
                } = def;
                ToolDefinition {
                    name: (*name).to_owned(),
                    description: (*description).to_owned(),
                    input_schema: input_schema.clone(),
                    effect: tool_effect(*effect),
                    // They only drive the connected app.
                    open_world: false,
                }
            })
            .collect()
    }

    fn requires_approval(&self, name: &str, _arguments: &Map<String, Value>) -> bool {
        egui_inspection_tools::tool(name)
            .is_none_or(|def| tool_effect(def.effect).requires_approval())
    }

    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> futures::future::BoxFuture<'static, Result<Vec<ToolOutput>, ToolError>> {
        let transport = (self.transport)();
        let name = name.to_owned();
        async move {
            let transport = transport.ok_or("no app connected — call `viewer_connect` first")?;
            let output =
                egui_inspection_tools::call(transport.as_ref(), &tokio_sleep, &name, arguments)
                    .await;
            let outputs = output.content.into_iter().map(tool_output);
            if output.is_error {
                let message = outputs
                    .filter_map(|output| match output {
                        ToolOutput::Text(text) => Some(text),
                        ToolOutput::Image { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Err(message.into())
            } else {
                Ok(outputs.collect())
            }
        }
        .boxed()
    }
}

/// The egui toolset of the MCP server.
pub struct EguiToolset {
    tools: EguiToolbox,
}

impl EguiToolset {
    /// The toolset, driving whatever app `transport` finds at the time of each call.
    pub fn new(transport: TransportSource) -> Self {
        Self {
            tools: EguiToolbox::new(transport),
        }
    }

    /// Every tool of this toolset, as the agent sees it.
    pub fn tools(&self) -> Vec<Tool> {
        toolbox::to_mcp_tools(&self.tools)
    }

    /// The egui part of the MCP `instructions`.
    pub fn instructions() -> &'static str {
        INSTRUCTIONS
    }

    /// Run the tool `name`, and shape its outcome as an MCP tool result.
    pub async fn call(&self, name: &str, arguments: Option<Map<String, Value>>) -> CallToolResult {
        toolbox::call(&self.tools, name, arguments).await
    }
}

/// The timer `ui_wait_for` polls with.
fn tokio_sleep(duration: Duration) -> BoxFuture<'static, ()> {
    Box::pin(tokio::time::sleep(duration))
}

fn tool_effect(effect: Effect) -> ToolEffect {
    match effect {
        Effect::ReadOnly => ToolEffect::ReadOnly,
        Effect::Idempotent => ToolEffect::IdempotentNonDestructive,
        Effect::Cumulative => ToolEffect::NonDestructive,
        Effect::Destructive => ToolEffect::Destructive,
    }
}

fn tool_output(content: Content) -> ToolOutput {
    match content {
        Content::Text(text) => ToolOutput::Text(text),
        Content::Image { bytes, mime_type } => ToolOutput::Image {
            bytes: bytes.into(),
            mime_type,
        },
    }
}

#[cfg(test)]
mod tests {
    use egui_inspection::protocol::{Request, Response};

    use super::*;

    /// A connected app that answers nothing.
    struct Offline;

    impl Transport for Offline {
        fn request(&self, _req: Request) -> BoxFuture<'_, Result<Response, String>> {
            Box::pin(async { Err("offline".to_owned()) })
        }
    }

    fn toolbox(transport: Option<Arc<dyn Transport>>) -> EguiToolbox {
        EguiToolbox::new(Arc::new(move || transport.clone()))
    }

    #[tokio::test]
    async fn a_call_before_connecting_says_how_to_connect() {
        let err = toolbox(None)
            .call("ui_widget_tree", Map::new())
            .await
            .expect_err("no app connected");
        assert!(err.to_string().contains("viewer_connect"), "{err}");
    }

    #[tokio::test]
    async fn only_prefixed_names_are_tools() {
        let err = toolbox(Some(Arc::new(Offline)))
            .call("click", Map::new())
            .await
            .expect_err("no prefix");
        assert!(err.to_string().contains("unknown tool `click`"), "{err}");
        assert!(EguiToolbox::owns("ui_click"));
        assert!(!EguiToolbox::owns("click"));
    }

    #[test]
    fn only_tools_that_may_lose_state_need_approval() {
        let toolbox = toolbox(None);
        for (name, expected) in [
            ("click", true),
            ("ui_batch", true),
            ("ui_click", true),
            ("ui_hover", false),
            ("ui_screenshot", false),
            ("ui_widget_tree", false),
        ] {
            assert_eq!(
                toolbox.requires_approval(name, &Map::new()),
                expected,
                "{name}"
            );
        }
    }
}
