//! [`CatalogTool`]: the tools, their names, descriptions, schemas, and effects.

use schemars::JsonSchema;
use serde_json::{Map, Value};
use strum::VariantArray as _;

use re_agent_tools::{ToolDefinition, ToolEffect};

use crate::args::{
    ConnectArgs, DatasetArgs, DatasetSchemaArgs, ListEntriesArgs, ListSegmentsArgs,
    SegmentPropertiesArgs,
};

/// One tool an agent can call.
///
/// [`strum::VariantArray::VARIANTS`] lists them in the order an agent should meet them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::VariantArray)]
pub enum CatalogTool {
    /// Pick the catalog server the other tools act on.
    Connect,

    /// List the catalog's datasets and tables.
    ListEntries,

    /// Summarize a dataset without reading its data.
    GetDatasetInfo,

    /// The entities, archetypes, and components of a dataset.
    GetDatasetSchema,

    /// Page through a dataset's segment ids.
    ListSegments,

    /// Read one segment's properties.
    GetSegmentProperties,
}

impl CatalogTool {
    /// The name the agent calls the tool by, e.g. `catalog_list_entries`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Connect => "catalog_connect",
            Self::ListEntries => "catalog_list_entries",
            Self::GetDatasetInfo => "catalog_get_dataset_info",
            Self::GetDatasetSchema => "catalog_get_dataset_schema",
            Self::ListSegments => "catalog_list_segments",
            Self::GetSegmentProperties => "catalog_get_segment_properties",
        }
    }

    /// The tool the agent calls `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::VARIANTS
            .iter()
            .copied()
            .find(|tool| tool.name() == name)
    }

    /// The tool's definition, as the agent sees it.
    pub fn definition(self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_owned(),
            description: self.description().to_owned(),
            input_schema: self.input_schema(),
            effect: self.effect(),
        }
    }

    /// Whether the user is asked before the agent runs the tool.
    ///
    /// `connect` asks although it is not destructive: it reaches any server the agent names,
    /// with the user's credentials.
    pub fn requires_approval(self) -> bool {
        self == Self::Connect || self.effect().requires_approval()
    }

    fn description(self) -> &'static str {
        match self {
            Self::Connect => {
                "Connect to a Rerun catalog server, replacing any previous catalog connection. \
                 The other catalog tools act on this catalog. Returns the server's version."
            }
            Self::ListEntries => {
                "List the datasets and tables in the catalog, sorted by name, a page at a time, \
                 with their ids and names, and the catalog they are on."
            }
            Self::GetDatasetInfo => {
                "Summarize a dataset without reading its data: number of segments, total size, \
                 layers, timelines with their time ranges, number of entities and component \
                 columns, and the segment property columns."
            }
            Self::GetDatasetSchema => {
                "The entities of a dataset and their archetypes, merged over all its segments, \
                 200 at a time (page on with `offset`). Pass `entity_path` for the components of \
                 one entity, with their Arrow datatypes. Entities under `/__properties` hold the \
                 static properties logged with each recording."
            }
            Self::ListSegments => {
                "List the ids of a dataset's segments (its recordings), sorted, a page at a time."
            }
            Self::GetSegmentProperties => {
                "The properties stored on the catalog for one segment, as \
                 `property:<key>:<component>` columns. Unlike the `/__properties` entities, these \
                 can change after the recording was registered."
            }
        }
    }

    fn effect(self) -> ToolEffect {
        match self {
            // Changes only which catalog the tools act on.
            Self::Connect => ToolEffect::NonDestructive,
            Self::ListEntries
            | Self::GetDatasetInfo
            | Self::GetDatasetSchema
            | Self::ListSegments
            | Self::GetSegmentProperties => ToolEffect::ReadOnly,
        }
    }

    fn input_schema(self) -> Map<String, Value> {
        match self {
            Self::Connect => schema_for::<ConnectArgs>(),
            Self::ListEntries => schema_for::<ListEntriesArgs>(),
            Self::GetDatasetInfo => schema_for::<DatasetArgs>(),
            Self::GetDatasetSchema => schema_for::<DatasetSchemaArgs>(),
            Self::ListSegments => schema_for::<ListSegmentsArgs>(),
            Self::GetSegmentProperties => schema_for::<SegmentPropertiesArgs>(),
        }
    }
}

/// The JSON Schema of a tool's arguments, derived from its `Args` struct in `args.rs`.
///
/// `schemars` turns the struct's fields and doc comments into a JSON Schema (draft 2020-12, the
/// version MCP uses), so the doc comments there are the field descriptions the agent reads.
/// Subschemas are inlined, since some model APIs handle `$ref` poorly. `$schema` and `title` are
/// dropped: tool schemas don't use them, and the title would be the Rust type name.
fn schema_for<Args: JsonSchema>() -> Map<String, Value> {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<Args>();
    // A struct's root schema is always an object.
    let Value::Object(mut input_schema) = schema.to_value() else {
        return Map::new();
    };
    input_schema.remove("$schema");
    input_schema.remove("title");
    input_schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_connect_asks_first() {
        let asking: Vec<_> = CatalogTool::VARIANTS
            .iter()
            .copied()
            .filter(|tool| tool.requires_approval())
            .collect();
        assert_eq!(asking, [CatalogTool::Connect]);
    }

    /// The agent-facing surface: names, descriptions, input schemas, and effects.
    #[test]
    fn definitions_snapshot() {
        let definitions: Vec<Value> = CatalogTool::VARIANTS
            .iter()
            .map(|tool| {
                let ToolDefinition {
                    name,
                    description,
                    input_schema,
                    effect,
                } = tool.definition();
                serde_json::json!({
                    "name": name,
                    "description": description,
                    "input_schema": input_schema,
                    "effect": format!("{effect:?}"),
                })
            })
            .collect();
        insta::assert_json_snapshot!(definitions);
    }
}
