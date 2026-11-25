//! Serves any [`AgentToolbox`] over MCP: its definitions become MCP tools, and its outputs MCP
//! content.

use std::sync::Arc;

use base64::Engine as _;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{Map, Value};

use re_agent_tools::{AgentToolbox, ToolDefinition, ToolOutput};

use crate::{annotations, text_error};

/// Convert the tool definitions of `toolbox` to MCP tools.
pub fn to_mcp_tools(toolbox: &dyn AgentToolbox) -> Vec<Tool> {
    toolbox.definitions().into_iter().map(to_mcp_tool).collect()
}

/// Convert `definition` to an MCP tool.
fn to_mcp_tool(definition: ToolDefinition) -> Tool {
    let ToolDefinition {
        name,
        description,
        input_schema,
        effect,
        open_world,
    } = definition;
    Tool::new(name, description, Arc::new(input_schema))
        .with_annotations(annotations(effect, open_world))
}

/// Run the tool `name` of `toolbox`, and shape its outcome as an MCP tool result.
pub async fn call(
    toolbox: &dyn AgentToolbox,
    name: &str,
    arguments: Option<Map<String, Value>>,
) -> CallToolResult {
    // Boxed: some toolboxes' futures are deep enough that inlining them into the server's future
    // overflows the compiler's query depth limit in the `rerun` binary.
    match Box::pin(toolbox.call(name, arguments.unwrap_or_default())).await {
        Ok(outputs) => CallToolResult::success(outputs.into_iter().map(content).collect()),
        Err(err) => text_error(err.to_string()),
    }
}

fn content(output: ToolOutput) -> ContentBlock {
    match output {
        ToolOutput::Text(text) => ContentBlock::text(text),
        ToolOutput::Image { bytes, mime_type } => ContentBlock::image(
            base64::engine::general_purpose::STANDARD.encode(bytes),
            mime_type,
        ),
    }
}
