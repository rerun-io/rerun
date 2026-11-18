//! [`CombinedToolbox`]: several toolboxes offered as one.

use std::sync::Arc;

use futures::future::BoxFuture;
use serde_json::{Map, Value};

use crate::{AgentToolbox, ToolDefinition, ToolError, ToolOutput};

/// Several toolboxes offered as one, e.g. the viewer's tools next to the catalog's.
///
/// Tool names must be unique across the toolboxes; the first toolbox with a name wins.
pub struct CombinedToolbox {
    toolboxes: Vec<Arc<dyn AgentToolbox>>,

    /// Which toolbox answers each tool name, from the definitions at construction.
    owner_of: ahash::HashMap<String, usize>,

    mcp_server_name: Option<String>,
}

impl CombinedToolbox {
    /// The tools of `toolboxes`, as one toolbox.
    ///
    /// `mcp_server_name` names the MCP server that serves all of these tools, if any.
    /// The tools of each toolbox are fixed at construction: a toolbox whose definitions change
    /// later answers only for the names it had then.
    pub fn new(
        mcp_server_name: Option<String>,
        toolboxes: impl IntoIterator<Item = Arc<dyn AgentToolbox>>,
    ) -> Self {
        let toolboxes: Vec<_> = toolboxes.into_iter().collect();
        let mut owner_of = ahash::HashMap::default();
        for (index, toolbox) in toolboxes.iter().enumerate() {
            for ToolDefinition { name, .. } in toolbox.definitions() {
                re_log::debug_assert!(
                    !owner_of.contains_key(&name),
                    "Two toolboxes define the tool `{name}`"
                );
                owner_of.entry(name).or_insert(index);
            }
        }
        Self {
            toolboxes,
            owner_of,
            mcp_server_name,
        }
    }

    fn owner(&self, name: &str) -> Option<&Arc<dyn AgentToolbox>> {
        let &index = self.owner_of.get(name)?;
        self.toolboxes.get(index)
    }
}

impl AgentToolbox for CombinedToolbox {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.toolboxes
            .iter()
            .enumerate()
            .flat_map(|(index, toolbox)| {
                toolbox
                    .definitions()
                    .into_iter()
                    .filter(move |definition| self.owner_of.get(&definition.name) == Some(&index))
            })
            .collect()
    }

    fn requires_approval(&self, name: &str, arguments: &Map<String, Value>) -> bool {
        self.owner(name)
            .is_none_or(|toolbox| toolbox.requires_approval(name, arguments))
    }

    fn mcp_server_name(&self) -> Option<&str> {
        self.mcp_server_name.as_deref()
    }

    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'static, Result<Vec<ToolOutput>, ToolError>> {
        if let Some(toolbox) = self.owner(name) {
            toolbox.call(name, arguments)
        } else {
            let err: ToolError = format!("Unknown tool `{name}`").into();
            Box::pin(std::future::ready(Err(err)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolEffect;

    /// Answers every call with its own `label`, and asks before every call when `asks`.
    struct Labeled {
        label: &'static str,
        tools: &'static [&'static str],
        asks: bool,
    }

    impl AgentToolbox for Labeled {
        fn definitions(&self) -> Vec<ToolDefinition> {
            self.tools
                .iter()
                .map(|name| ToolDefinition {
                    name: (*name).to_owned(),
                    description: String::new(),
                    input_schema: Map::new(),
                    effect: ToolEffect::ReadOnly,
                })
                .collect()
        }

        fn requires_approval(&self, _name: &str, _arguments: &Map<String, Value>) -> bool {
            self.asks
        }

        fn call(
            &self,
            _name: &str,
            _arguments: Map<String, Value>,
        ) -> BoxFuture<'static, Result<Vec<ToolOutput>, ToolError>> {
            let label = self.label;
            Box::pin(async move { Ok(vec![ToolOutput::Text(label.to_owned())]) })
        }
    }

    fn viewer_and_catalog() -> CombinedToolbox {
        CombinedToolbox::new(
            Some("rerun".to_owned()),
            [
                Arc::new(Labeled {
                    label: "viewer",
                    tools: &["viewer_get_state"],
                    asks: true,
                }) as Arc<dyn AgentToolbox>,
                Arc::new(Labeled {
                    label: "catalog",
                    tools: &["catalog_sql"],
                    asks: false,
                }),
            ],
        )
    }

    fn call_text(toolbox: &CombinedToolbox, name: &str) -> Result<String, String> {
        match pollster::block_on(toolbox.call(name, Map::new())) {
            Ok(outputs) => match outputs.as_slice() {
                [ToolOutput::Text(text)] => Ok(text.clone()),
                _ => Err("unexpected output".to_owned()),
            },
            Err(err) => Err(err.to_string()),
        }
    }

    #[test]
    fn lists_every_tool_in_toolbox_order() {
        let names: Vec<String> = viewer_and_catalog()
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert_eq!(names, ["viewer_get_state", "catalog_sql"]);
    }

    #[test]
    fn calls_reach_the_owning_toolbox() {
        let combined = viewer_and_catalog();
        assert_eq!(
            call_text(&combined, "catalog_sql"),
            Ok("catalog".to_owned())
        );
        assert_eq!(
            call_text(&combined, "viewer_get_state"),
            Ok("viewer".to_owned())
        );
    }

    #[test]
    fn approval_follows_the_owning_toolbox() {
        let combined = viewer_and_catalog();
        assert!(combined.requires_approval("viewer_get_state", &Map::new()));
        assert!(!combined.requires_approval("catalog_sql", &Map::new()));
    }

    #[test]
    fn unknown_tools_fail_and_ask() {
        let combined = viewer_and_catalog();
        assert!(call_text(&combined, "nope").is_err_and(|err| err.contains("`nope`")));
        assert!(combined.requires_approval("nope", &Map::new()));
    }
}
