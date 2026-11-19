//! The arguments of each tool, whose doc comments become the agent-facing field descriptions.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectArgs {
    /// URL of the catalog server, e.g. `rerun+http://127.0.0.1:51234`. `http(s)://` works too.
    pub url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginArgs {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EntryKindArg {
    Dataset,
    Table,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListEntriesArgs {
    /// Only list entries of this kind. Lists datasets and tables when left out.
    #[serde(default)]
    pub kind: Option<EntryKindArg>,

    /// How many entries to skip.
    #[serde(default)]
    pub offset: usize,

    /// How many entries to return: 100 by default, at most 1000.
    #[serde(default = "ListEntriesArgs::default_entry_page")]
    pub limit: usize,
}

impl ListEntriesArgs {
    /// How many entries `list_entries` returns when the agent does not say.
    pub const DEFAULT_ENTRY_PAGE: usize = 100;

    /// The most entries `list_entries` returns in one page, so a catalog with many entries does
    /// not flood the agent's context in one call.
    pub const MAX_ENTRY_PAGE: usize = 1000;

    fn default_entry_page() -> usize {
        Self::DEFAULT_ENTRY_PAGE
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatasetArgs {
    /// The dataset's name or id, from `catalog_list_entries`.
    pub dataset: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatasetSchemaArgs {
    /// The dataset's name or id, from `catalog_list_entries`.
    pub dataset: String,

    /// One entity to describe in full, e.g. `/camera/image`.
    #[serde(default)]
    pub entity_path: Option<String>,

    /// Without `entity_path`: how many entities to skip, to list those past the first 200.
    #[serde(default)]
    pub offset: usize,
}

impl DatasetSchemaArgs {
    /// The most entities `get_dataset_schema` lists in one page without an `entity_path`.
    ///
    /// A robot recording rarely has more; past this, the agent pages on with `offset`.
    pub const MAX_SCHEMA_ENTITIES: usize = 200;
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListSegmentsArgs {
    /// The dataset's name or id, from `catalog_list_entries`.
    pub dataset: String,

    /// How many segments to skip.
    #[serde(default)]
    pub offset: usize,

    /// How many segment ids to return: 100 by default, at most 1000.
    #[serde(default = "ListSegmentsArgs::default_segment_page")]
    pub limit: usize,
}

impl ListSegmentsArgs {
    /// How many segment ids `list_segments` returns when the agent does not say.
    pub const DEFAULT_SEGMENT_PAGE: usize = 100;

    /// The most segment ids `list_segments` returns in one page.
    ///
    /// About 40 kB of ids: enough to scan a dataset in a few calls without flooding the agent's
    /// context in one.
    pub const MAX_SEGMENT_PAGE: usize = 1000;

    fn default_segment_page() -> usize {
        Self::DEFAULT_SEGMENT_PAGE
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SegmentPropertiesArgs {
    /// The dataset's name or id, from `catalog_list_entries`.
    pub dataset: String,

    /// The segment's id, from `catalog_list_segments`.
    pub segment_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryTablesArgs {
    /// A DataFusion SQL query over the catalog's table entries, by their names from
    /// `catalog_list_entries`, e.g. `SELECT * FROM my_table WHERE score > 0.5`.
    pub sql: String,

    /// The most rows to return: 50 by default, at most 1000.
    #[serde(default = "default_max_rows")]
    pub max_rows: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryDatasetArgs {
    /// The dataset's name or id, from `catalog_list_entries`.
    pub dataset: String,

    /// The timeline to index rows by, from `catalog_get_dataset_info`. May be left out when the
    /// dataset has a single timeline. A dataset without timelines returns its static data.
    #[serde(default)]
    pub index: Option<String>,

    /// The segments to query, from `catalog_list_segments`. All of them when left out.
    #[serde(default)]
    pub segments: Option<Vec<String>>,

    /// The entities whose components become columns, from `catalog_get_dataset_schema`. All
    /// entities when left out.
    #[serde(default)]
    pub entity_paths: Option<Vec<String>>,

    /// A DataFusion SQL query over the result, as a table named `dataset`, e.g.
    /// `SELECT rerun_segment_id, count(*) FROM dataset GROUP BY rerun_segment_id`.
    /// Defaults to `SELECT * FROM dataset`.
    #[serde(default)]
    pub sql: Option<String>,

    /// The most rows to return: 50 by default, at most 1000.
    #[serde(default = "default_max_rows")]
    pub max_rows: usize,
}

fn default_max_rows() -> usize {
    crate::query::DEFAULT_MAX_ROWS
}
