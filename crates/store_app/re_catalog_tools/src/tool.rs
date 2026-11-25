//! [`CatalogTool`]: the tools, their names, descriptions, schemas, and effects.

use schemars::JsonSchema;
use serde_json::{Map, Value};
use strum::VariantArray as _;

use re_agent_tools::{ToolDefinition, ToolEffect};

use crate::args::{
    ConnectArgs, CreateDatasetsArgs, DatasetArgs, DatasetSchemaArgs, ListEntriesArgs,
    ListSegmentsArgs, ListServersArgs, LoginArgs, QueryDatasetArgs, QueryTablesArgs,
    RegisterRecordingsArgs, RegisterTableBlueprintArgs, RenameEntriesArgs, SegmentPropertiesArgs,
    UploadRecordingsArgs,
};

/// One tool an agent can call.
///
/// [`strum::VariantArray::VARIANTS`] lists them in the order an agent should meet them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::VariantArray)]
pub enum CatalogTool {
    /// List the catalog servers the user has added, to pick one to connect to.
    ListServers,

    /// Pick the catalog server the other tools act on.
    Connect,

    /// Log the user in, for a catalog server that needs it.
    Login,

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

    /// Read the logged data of a dataset's recordings, one row per index value per segment,
    /// with optional SQL over that one table.
    QueryDataset,

    /// Run SQL over the catalog's table entries (plain Arrow tables, such as `__entries`), which
    /// hold no recordings.
    QueryTables,

    /// Create empty datasets.
    CreateDatasets,

    /// Rename datasets and tables.
    RenameEntries,

    /// Upload local `.rrd` files into a dataset.
    UploadRecordings,

    /// Register `.rrd` files the server can already read into a dataset.
    RegisterRecordings,

    /// Register an `.rbl` table blueprint the server can read, optionally as the default.
    RegisterTableBlueprint,
}

impl CatalogTool {
    /// The name the agent calls the tool by, e.g. `catalog_list_entries`.
    pub fn name(self) -> &'static str {
        match self {
            Self::ListServers => "catalog_list_servers",
            Self::Connect => "catalog_connect",
            Self::Login => "catalog_login",
            Self::ListEntries => "catalog_list_entries",
            Self::GetDatasetInfo => "catalog_get_dataset_info",
            Self::GetDatasetSchema => "catalog_get_dataset_schema",
            Self::ListSegments => "catalog_list_segments",
            Self::GetSegmentProperties => "catalog_get_segment_properties",
            Self::QueryDataset => "catalog_query_dataset",
            Self::QueryTables => "catalog_query_tables",
            Self::CreateDatasets => "catalog_create_datasets",
            Self::RenameEntries => "catalog_rename_entries",
            Self::UploadRecordings => "catalog_upload_recordings",
            Self::RegisterRecordings => "catalog_register_recordings",
            Self::RegisterTableBlueprint => "catalog_register_table_blueprint",
        }
    }

    /// The tool the agent calls `name`, if it is available here.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::available().find(|tool| tool.name() == name)
    }

    /// The tools available on this platform, in the order an agent should meet them.
    ///
    /// `list_servers` needs the list on disk, which the browser has none of. `login` needs a local
    /// web server for the login callback, which the browser cannot start, and `upload_recordings`
    /// reads a local file, which the browser cannot do.
    pub fn available() -> impl Iterator<Item = Self> {
        Self::VARIANTS
            .iter()
            .copied()
            .filter(|tool| !cfg!(target_arch = "wasm32") || !tool.is_native_only())
    }

    fn is_native_only(self) -> bool {
        match self {
            Self::ListServers | Self::Login | Self::UploadRecordings => true,
            Self::Connect
            | Self::ListEntries
            | Self::GetDatasetInfo
            | Self::GetDatasetSchema
            | Self::ListSegments
            | Self::GetSegmentProperties
            | Self::QueryDataset
            | Self::QueryTables
            | Self::CreateDatasets
            | Self::RenameEntries
            | Self::RegisterRecordings
            | Self::RegisterTableBlueprint => false,
        }
    }

    /// Whether the tool changes the catalog. These always ask the user first.
    ///
    /// None of them removes or replaces data, since a catalog has no way to undo that.
    pub fn writes(self) -> bool {
        match self {
            Self::CreateDatasets
            | Self::RenameEntries
            | Self::UploadRecordings
            | Self::RegisterRecordings
            | Self::RegisterTableBlueprint => true,
            Self::ListServers
            | Self::Connect
            | Self::Login
            | Self::ListEntries
            | Self::GetDatasetInfo
            | Self::GetDatasetSchema
            | Self::ListSegments
            | Self::GetSegmentProperties
            | Self::QueryDataset
            | Self::QueryTables => false,
        }
    }

    /// The tool's definition, as the agent sees it.
    pub fn definition(self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_owned(),
            description: self.description().to_owned(),
            input_schema: self.input_schema(),
            effect: self.effect(),
            // Every tool talks to a catalog server, which may be anywhere.
            open_world: true,
        }
    }

    /// Whether the tool acts on the catalog the tools are connected to.
    ///
    /// `list_servers` lists the ones to pick from, `connect` picks a catalog itself, and `login`
    /// logs the user in for any catalog.
    pub fn acts_on_catalog(self) -> bool {
        match self {
            Self::ListServers | Self::Connect | Self::Login => false,
            Self::ListEntries
            | Self::GetDatasetInfo
            | Self::GetDatasetSchema
            | Self::ListSegments
            | Self::GetSegmentProperties
            | Self::QueryDataset
            | Self::QueryTables
            | Self::CreateDatasets
            | Self::RenameEntries
            | Self::UploadRecordings
            | Self::RegisterRecordings
            | Self::RegisterTableBlueprint => true,
        }
    }

    /// `connect` and `login`, which set up the session.
    fn is_session_tool(self) -> bool {
        match self {
            Self::Connect | Self::Login => true,
            Self::ListServers
            | Self::ListEntries
            | Self::GetDatasetInfo
            | Self::GetDatasetSchema
            | Self::ListSegments
            | Self::GetSegmentProperties
            | Self::QueryDataset
            | Self::QueryTables
            | Self::CreateDatasets
            | Self::RenameEntries
            | Self::UploadRecordings
            | Self::RegisterRecordings
            | Self::RegisterTableBlueprint => false,
        }
    }

    /// What an agent that may call the write tools should know, for the host's instructions.
    pub fn write_guidance() -> String {
        let mut guidance = format!(
            "The write tools, such as `{create}`, change the catalog for everyone using it. \
             Confirm the server with the user before you change anything.",
            create = Self::CreateDatasets.name(),
        );
        if !cfg!(target_arch = "wasm32") || !Self::UploadRecordings.is_native_only() {
            guidance.push_str(" Upload `.rrd` files on this machine with `");
            guidance.push_str(Self::UploadRecordings.name());
            guidance.push_str("`.");
        }
        guidance
    }

    /// Whether the user is asked before the agent runs the tool.
    ///
    /// `connect` asks although it is not destructive: it reaches any server the agent names,
    /// with the user's credentials. `login` asks because it saves new credentials on the machine,
    /// once the user logs in through their browser. Every tool that writes asks, since its
    /// changes are shared with everyone using the catalog.
    pub fn requires_approval(self) -> bool {
        self.is_session_tool() || self.writes() || self.effect().requires_approval()
    }

    fn description(self) -> &'static str {
        match self {
            Self::ListServers => {
                "List the catalog servers the user has added, in the Rerun viewer or with \
                 `rerun servers add`, and the one the catalog tools act on now. Pass one to \
                 `catalog_connect`."
            }
            Self::Connect => {
                "Connect to a Rerun catalog server, replacing any previous catalog connection. \
                 The other catalog tools act on this catalog. Returns the server's version."
            }
            Self::Login => {
                "Log the user in on this machine, for a catalog server that needs a login. The \
                 first call returns a link: show it to the user and ask them to open it. Call \
                 again to wait until they are done, then call `catalog_connect` again. Returns \
                 `logged_in` at once if the user is already logged in."
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
            Self::QueryDataset => {
                "Read the logged data of one dataset's recordings as a table: one row per index \
                 value per segment, with a `rerun_segment_id` column, the index column, and one \
                 column per component of the chosen entities. Narrow it with `segments`, \
                 `entity_paths`, and `sql`, which runs over that table. Rows are arrays in \
                 `columns` order; long values (images, point clouds) are cut short or left out. For the catalog's \
                 table entries, use `catalog_query_tables`."
            }
            Self::QueryTables => {
                "Run a read-only DataFusion SQL query over the catalog's table entries, by their \
                 names from `catalog_list_entries`, e.g. `__entries`, which lists every entry. \
                 Quote names with capitals or dots. Datasets are not table entries: read their \
                 data with `catalog_query_dataset`."
            }
            Self::CreateDatasets => {
                "Create empty datasets in the catalog. Put all of them in one call, so the user \
                 approves them at once. Checks every name first, then creates them in order, and \
                 stops at the first the server refuses. Fill them with \
                 `catalog_upload_recordings` or `catalog_register_recordings`."
            }
            Self::RenameEntries => {
                "Rename datasets and tables in the catalog. Put all renames in one call, so the \
                 user approves them at once. Checks them all first, then renames in order, and \
                 stops at the first the server refuses."
            }
            Self::UploadRecordings => {
                "Upload `.rrd` files on this machine into a dataset, and wait for the server to \
                 register their recordings as segments. Put all files in one call, so the user \
                 approves them at once. Checks every file first, then uploads them in order, and \
                 stops at the first that fails. Returns the segment ids, with `skip` also those \
                 already there. Large files take a while to upload."
            }
            Self::RegisterRecordings => {
                "Register `.rrd` files that the catalog server can read, e.g. on S3, into a \
                 dataset, without copying them. Waits for the server to register their \
                 recordings as segments, and returns the segment ids, with `skip` also those \
                 already there. For files on this machine, use `catalog_upload_recordings`."
            }
            Self::RegisterTableBlueprint => {
                "Register an `.rbl` table blueprint that the catalog server can read, e.g. on S3, \
                 for a dataset's segment table, under a new name. With `set_default`, it also \
                 becomes the table's default for everyone who opens the dataset; the previous \
                 default stays registered. Never replaces a registered blueprint."
            }
        }
    }

    fn effect(self) -> ToolEffect {
        match self {
            // A rename is undone by renaming back, and uploading and registering add segments
            // without replacing any.
            Self::Connect
            | Self::Login
            | Self::CreateDatasets
            | Self::RenameEntries
            | Self::UploadRecordings
            | Self::RegisterRecordings
            | Self::RegisterTableBlueprint => ToolEffect::NonDestructive,
            Self::ListServers
            | Self::ListEntries
            | Self::GetDatasetInfo
            | Self::GetDatasetSchema
            | Self::ListSegments
            | Self::GetSegmentProperties
            | Self::QueryDataset
            | Self::QueryTables => ToolEffect::ReadOnly,
        }
    }

    fn input_schema(self) -> Map<String, Value> {
        match self {
            Self::ListServers => schema_for::<ListServersArgs>(),
            Self::Connect => schema_for::<ConnectArgs>(),
            Self::Login => schema_for::<LoginArgs>(),
            Self::ListEntries => schema_for::<ListEntriesArgs>(),
            Self::GetDatasetInfo => schema_for::<DatasetArgs>(),
            Self::GetDatasetSchema => schema_for::<DatasetSchemaArgs>(),
            Self::ListSegments => schema_for::<ListSegmentsArgs>(),
            Self::GetSegmentProperties => schema_for::<SegmentPropertiesArgs>(),
            Self::QueryDataset => schema_for::<QueryDatasetArgs>(),
            Self::QueryTables => schema_for::<QueryTablesArgs>(),
            Self::CreateDatasets => schema_for::<CreateDatasetsArgs>(),
            Self::RenameEntries => schema_for::<RenameEntriesArgs>(),
            Self::UploadRecordings => schema_for::<UploadRecordingsArgs>(),
            Self::RegisterRecordings => schema_for::<RegisterRecordingsArgs>(),
            Self::RegisterTableBlueprint => schema_for::<RegisterTableBlueprintArgs>(),
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
    // Some function-calling APIs reject an object schema without `properties`.
    input_schema
        .entry("properties")
        .or_insert_with(|| Value::Object(Map::new()));
    input_schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_login_and_the_writes_ask_first() {
        let asking: Vec<_> = CatalogTool::VARIANTS
            .iter()
            .copied()
            .filter(|tool| tool.requires_approval())
            .collect();
        let writes: Vec<_> = CatalogTool::VARIANTS
            .iter()
            .copied()
            .filter(|tool| tool.writes())
            .collect();
        assert_eq!(
            asking,
            [
                [CatalogTool::Connect, CatalogTool::Login].as_slice(),
                &writes
            ]
            .concat()
        );
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
                    open_world: _,
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
