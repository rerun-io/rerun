//! The tools that read data: SQL over the catalog's tables, and queries of a dataset's contents.
//!
//! Both run agent-written SQL, so the session only allows queries (no DDL, DML, or statements),
//! has a memory limit, never spills to disk, runs under a deadline, and stops reading results once
//! the response is full.

use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::TableReference;
use datafusion::execution::SendableRecordBatchStream;
use datafusion::execution::context::SQLOptions;
use datafusion::execution::disk_manager::{DiskManagerBuilder, DiskManagerMode};
use datafusion::execution::memory_pool::{GreedyMemoryPool, MemoryPool};
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::prelude::{SessionConfig, SessionContext};
use futures::StreamExt as _;
use serde_json::{Value, json};

use re_catalog_client::CatalogClient;
use re_chunk_store::{QueryExpression, StaticColumnSelection};
use re_datafusion::{DataframeQueryTableProvider, TableEntryTableProvider};
use re_log_types::{EntityPath, TimelineName};
use re_protos::cloud::v1alpha1::DatasetQueryScope;

use crate::CatalogToolError;
use crate::args::{QueryDatasetArgs, QueryTablesArgs};

/// How many rows a query returns when the agent does not say.
pub const DEFAULT_MAX_ROWS: usize = 50;

/// The most rows a query returns, whatever the agent asks for.
///
/// More rows than this do not fit in [`MAX_RESULT_BYTES`] unless they are tiny, and an agent that
/// wants more should aggregate in SQL instead.
const MAX_ROWS: usize = 1000;

/// The most bytes of JSON a query returns, column names included. Rows past it are dropped, and
/// the result says so.
///
/// About 10k tokens: enough to read a result, small enough to leave room in the agent's context.
const MAX_RESULT_BYTES: usize = 40_000;

/// Strings longer than this many characters are cut in results.
const MAX_STRING_CHARS: usize = 200;

/// Arrays longer than this are cut in results, e.g. the pixels of an image.
const MAX_ARRAY_ITEMS: usize = 16;

/// Cells whose Arrow data is larger than this are not encoded at all, e.g. an image or a point
/// cloud, since encoding them as JSON only to cut them would cost far more than the rest of the
/// row.
const MAX_CELL_BYTES: usize = 64 << 10;

/// The most memory the queries of all tool calls together may use for their operators (sorts,
/// joins, aggregations).
///
/// The dataset scan itself buffers outside this limit, which is why queries run in one
/// partition. Smaller in the browser, where the whole tab has a few GiB of address space.
const MAX_QUERY_MEMORY_BYTES: usize = cfg_select! {
    target_arch = "wasm32" => 512 << 20,
    _ => 2 << 30,
};

/// How long one tool call may take, from opening its tables to encoding the last row.
///
/// Long enough for a scan of a few segments of a remote dataset, short enough that an agent
/// that asked for too much hears back while the user is still waiting.
const QUERY_TIMEOUT: Duration = Duration::from_secs(120);

/// The most tables one `catalog_query_tables` query may name, since each one is opened with a
/// request to the catalog before the query runs.
const MAX_TABLES: usize = 16;

/// Room kept in [`MAX_RESULT_BYTES`] for the `truncated` message and the JSON around the rows.
const RESULT_OVERHEAD_BYTES: usize = 300;

/// The table name a dataset query is registered under, for its `sql`.
const DATASET_TABLE: &str = "dataset";

/// Run the agent's SQL over the catalog's tables.
pub async fn query_tables(
    catalog: &CatalogClient,
    args: QueryTablesArgs,
) -> Result<Value, CatalogToolError> {
    // Boxed: DataFusion's futures are large.
    Box::pin(with_deadline(
        "Aggregate in SQL, or name fewer tables.",
        query_tables_within_deadline(catalog, args),
    ))
    .await
}

async fn query_tables_within_deadline(
    catalog: &CatalogClient,
    args: QueryTablesArgs,
) -> Result<Value, CatalogToolError> {
    let QueryTablesArgs { sql, max_rows } = args;
    let connection = catalog
        .connection()
        .connection()
        .await
        .map_err(|err| CatalogToolError::catalog("connect to the catalog", err))?;
    let ctx = session()?;

    // Only the tables the query names are registered, each resolved without blocking, so this
    // also works in the browser.
    let state = ctx.state();
    let statement = state
        .sql_to_statement(&sql, &state.config_options().sql_parser.dialect)
        .map_err(|source| CatalogToolError::InvalidQuery { source })?;
    let references = state
        .resolve_table_references(&statement)
        .map_err(|source| CatalogToolError::InvalidQuery { source })?;
    let mut names = Vec::new();
    for reference in references {
        // Table functions, e.g. `generate_series(1, 10)`, show up as table references too.
        if state.table_functions().contains_key(reference.table()) {
            continue;
        }
        if !matches!(reference, TableReference::Bare { .. }) {
            return Err(CatalogToolError::DottedTableName { reference });
        }
        names.push(reference.table().to_owned());
    }
    if MAX_TABLES < names.len() {
        return Err(CatalogToolError::TooManyTables {
            num_tables: names.len(),
            max_tables: MAX_TABLES,
        });
    }
    let providers = futures::future::try_join_all(names.iter().map(|name| {
        let name = name.clone();
        let opened = TableEntryTableProvider::new(connection.client.clone(), name.clone(), None)
            .into_provider();
        async move {
            opened.await.map_err(|source| CatalogToolError::OpenTable {
                table: name,
                source,
            })
        }
    }))
    .await?;
    for (name, provider) in std::iter::zip(names, providers) {
        ctx.register_table(TableReference::bare(name), provider)
            .map_err(|source| CatalogToolError::SetUpQuery { source })?;
    }
    collect_rows(&ctx, &sql, max_rows).await
}

/// Read a dataset's contents as a table, and run the agent's SQL over it.
pub async fn query_dataset(
    catalog: &CatalogClient,
    args: QueryDatasetArgs,
) -> Result<Value, CatalogToolError> {
    // Boxed: DataFusion's futures are large.
    Box::pin(with_deadline(
        "Narrow it to fewer segments, entities, or columns, or aggregate in its `sql`.",
        query_dataset_within_deadline(catalog, args),
    ))
    .await
}

async fn query_dataset_within_deadline(
    catalog: &CatalogClient,
    args: QueryDatasetArgs,
) -> Result<Value, CatalogToolError> {
    let QueryDatasetArgs {
        dataset,
        index,
        segments,
        entity_paths,
        sql,
        max_rows,
    } = args;
    let id = crate::metadata::dataset_id(catalog, &dataset).await?;
    if segments.as_ref().is_some_and(Vec::is_empty) {
        return Err(CatalogToolError::NoSegments);
    }
    if entity_paths.as_ref().is_some_and(Vec::is_empty) {
        return Err(CatalogToolError::NoEntities);
    }
    if let Some(segments) = &segments {
        check_segments_exist(catalog, id, segments).await?;
    }

    let schema = catalog
        .get_dataset_schema(id, DatasetQueryScope::Full)
        .await
        .map_err(|err| CatalogToolError::catalog("read the dataset schema", err))?
        .schema;
    let columns = re_sorbet::SorbetColumnDescriptors::try_from_arrow_fields(None, schema.fields())
        .map_err(|source| CatalogToolError::Schema { source })?;
    let index = resolve_index(&columns, index)?;
    let entity_paths = entity_paths
        .map(|paths| resolve_entity_paths(&columns, &paths))
        .transpose()?;

    let query = QueryExpression {
        view_contents: entity_paths
            .map(|paths| paths.into_iter().map(|path| (path, None)).collect()),
        include_static_columns: if index.is_some() {
            StaticColumnSelection::Both
        } else {
            StaticColumnSelection::StaticOnly
        },
        filtered_index: index,
        ..Default::default()
    };
    let segments = segments.unwrap_or_default();
    let provider = cfg_select! {
        target_arch = "wasm32" => DataframeQueryTableProvider::new(
            catalog.connection().clone(),
            id,
            &query,
            &segments,
            None,
            Some(schema),
            Vec::new(),
        ),
        _ => DataframeQueryTableProvider::new(
            catalog.connection().clone(),
            id,
            &query,
            &segments,
            None,
            Some(schema),
            None,
            Vec::new(),
            Arc::new(re_datafusion::NoOpObjectStoreAuthenticator::default()),
        ),
    }
    .await
    .map_err(|err| CatalogToolError::catalog("query the dataset", err))?;

    let ctx = session()?;
    ctx.register_table(DATASET_TABLE, Arc::new(provider))
        .map_err(|source| CatalogToolError::SetUpQuery { source })?;
    let sql = sql.unwrap_or_else(|| format!("SELECT * FROM {DATASET_TABLE}"));
    collect_rows(&ctx, &sql, max_rows).await
}

/// Fails with [`CatalogToolError::NoSuchSegment`] for the first of `segments` the dataset lacks,
/// since the query would silently return no rows for it.
async fn check_segments_exist(
    catalog: &CatalogClient,
    dataset_id: re_log_types::EntryId,
    segments: &[String],
) -> Result<(), CatalogToolError> {
    let known: std::collections::BTreeSet<String> = catalog
        .get_dataset_segment_ids(dataset_id)
        .await
        .map_err(|err| CatalogToolError::catalog("list the segments", err))?
        .into_iter()
        .map(|segment_id| segment_id.to_string())
        .collect();
    match segments.iter().find(|segment| !known.contains(*segment)) {
        Some(unknown) => Err(CatalogToolError::NoSuchSegment {
            segment_id: unknown.clone(),
        }),
        None => Ok(()),
    }
}

/// The timeline to index by: the one the agent named, else the dataset's only timeline.
///
/// `None` means a static-only query, for datasets with no timelines.
fn resolve_index(
    columns: &re_sorbet::SorbetColumnDescriptors,
    index: Option<String>,
) -> Result<Option<TimelineName>, CatalogToolError> {
    let timelines: Vec<TimelineName> = columns
        .index_columns()
        .map(|column| column.timeline_name())
        .collect();
    match (index, timelines.as_slice()) {
        (Some(index), _) => {
            let index = TimelineName::try_new(index)
                .map_err(|source| CatalogToolError::InvalidTimeline { source })?;
            if timelines.contains(&index) {
                Ok(Some(index))
            } else {
                Err(CatalogToolError::NoSuchIndex { index, timelines })
            }
        }
        (None, []) => Ok(None),
        (None, [timeline]) => Ok(Some(*timeline)),
        (None, _) => Err(CatalogToolError::AmbiguousIndex { timelines }),
    }
}

/// The entities the agent named, each of which must be in the dataset.
fn resolve_entity_paths(
    columns: &re_sorbet::SorbetColumnDescriptors,
    paths: &[String],
) -> Result<Vec<EntityPath>, CatalogToolError> {
    let known: std::collections::BTreeSet<&EntityPath> = columns
        .component_columns()
        .map(|column| &column.entity_path)
        .collect();
    paths
        .iter()
        .map(|path| {
            let entity_path = EntityPath::parse_forgiving(path);
            if known.contains(&entity_path) {
                Ok(entity_path)
            } else {
                Err(CatalogToolError::NoSuchEntity { entity_path })
            }
        })
        .collect()
}

/// A session that never spills to disk, and shares one memory limit with every other session,
/// so that concurrent tool calls cannot each use [`MAX_QUERY_MEMORY_BYTES`].
fn session() -> Result<SessionContext, CatalogToolError> {
    static MEMORY_POOL: std::sync::LazyLock<Arc<dyn MemoryPool>> =
        std::sync::LazyLock::new(|| Arc::new(GreedyMemoryPool::new(MAX_QUERY_MEMORY_BYTES)));
    let runtime = RuntimeEnvBuilder::new()
        .with_memory_pool(MEMORY_POOL.clone())
        .with_disk_manager_builder(
            DiskManagerBuilder::default().with_mode(DiskManagerMode::Disabled),
        )
        .build_arc()
        .map_err(|source| CatalogToolError::SetUpQuery { source })?;
    Ok(SessionContext::new_with_config_rt(
        SessionConfig::new().with_target_partitions(1),
        runtime,
    ))
}

/// Run `future` within [`QUERY_TIMEOUT`], suggesting `hint` when it runs out.
///
/// A timeout only interrupts the future while it waits. In the browser nothing else preempts
/// it, so [`read_rows`] yields there between batches; work within one batch, e.g. a large sort,
/// still runs to its end.
async fn with_deadline(
    hint: &'static str,
    future: impl Future<Output = Result<Value, CatalogToolError>>,
) -> Result<Value, CatalogToolError> {
    re_async::timeout(QUERY_TIMEOUT, future)
        .await
        .map_err(|_elapsed| CatalogToolError::QueryTimedOut {
            timeout: QUERY_TIMEOUT,
            hint,
        })?
}

/// Run the read-only `query`, and return at most `max_rows` of its rows as JSON within
/// [`MAX_RESULT_BYTES`].
async fn collect_rows(
    ctx: &SessionContext,
    query: &str,
    max_rows: usize,
) -> Result<Value, CatalogToolError> {
    let max_rows = max_rows.min(MAX_ROWS);
    let read_only = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);
    let frame = ctx
        .sql_with_options(query, read_only)
        .await
        .map_err(|source| CatalogToolError::InvalidQuery { source })?;
    let columns: Vec<String> = frame
        .schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();

    // One row past the limit, to tell whether the result was cut.
    let stream = frame
        .limit(0, Some(max_rows + 1))
        .map_err(|source| CatalogToolError::InvalidQuery { source })?
        .execute_stream()
        .await
        .map_err(|source| CatalogToolError::QueryFailed { source })?;

    let num_bytes = json!({ "columns": &columns, "rows": [] }).to_string().len();
    if MAX_RESULT_BYTES / 2 < num_bytes {
        return Err(CatalogToolError::TooManyColumns {
            num_columns: columns.len(),
        });
    }
    let (rows, truncated) = read_rows(stream, max_rows, num_bytes + RESULT_OVERHEAD_BYTES).await?;
    let mut result = json!({
        "columns": columns,
        "rows": rows,
    });
    if let Some(truncated) = truncated {
        result["truncated"] = json!(truncated);
    }
    Ok(result)
}

/// At most `max_rows` rows of `stream` as JSON, within [`MAX_RESULT_BYTES`] counting the
/// `num_bytes` already spent, and why reading stopped early, if it did.
async fn read_rows(
    mut stream: SendableRecordBatchStream,
    max_rows: usize,
    mut num_bytes: usize,
) -> Result<(Vec<Value>, Option<String>), CatalogToolError> {
    let mut rows = Vec::new();
    while let Some(batch) = stream.next().await {
        let batch = batch.map_err(|source| CatalogToolError::QueryFailed { source })?;
        for row_index in 0..batch.num_rows() {
            if rows.len() == max_rows {
                let raise = if max_rows < MAX_ROWS {
                    format!(", or raise `max_rows` (at most {MAX_ROWS})")
                } else {
                    String::new()
                };
                let truncated = format!(
                    "Returned the first {max_rows} rows; there are more. Filter or aggregate in \
                     SQL{raise}."
                );
                return Ok((rows, Some(truncated)));
            }
            let mut cells = row_json(&batch.slice(row_index, 1))?;
            // Each cell on its own: a row is an array too, but must keep every column.
            cells.iter_mut().for_each(shorten);
            let row = Value::Array(cells);
            // One more byte for the comma between rows.
            num_bytes += row.to_string().len() + 1;
            if MAX_RESULT_BYTES < num_bytes {
                let truncated = format!(
                    "Returned {} rows, as many as fit in the response. Select fewer columns, or \
                     filter or aggregate in SQL.",
                    rows.len()
                );
                return Ok((rows, Some(truncated)));
            }
            rows.push(row);
        }
        if cfg!(target_arch = "wasm32") {
            // Lets the deadline interrupt a query in the browser, where nothing else preempts it.
            re_async::yield_now().await;
        }
    }
    Ok((rows, None))
}

/// The values of the single row of `row`, in column order.
///
/// Cells JSON cannot encode (unions, e.g. tensor buffers) or that are too large to be worth
/// encoding become placeholder strings, so one such cell does not fail or flood the result.
fn row_json(row: &RecordBatch) -> Result<Vec<Value>, CatalogToolError> {
    // Named by position, since a query may return several columns of the same name, e.g. a join.
    let (fields, cells): (Vec<Field>, Vec<ArrayRef>) =
        std::iter::zip(row.schema().fields(), row.columns())
            .enumerate()
            .map(|(position, (field, cell))| {
                let name = position.to_string();
                let placeholder = if contains_union(field.data_type()) {
                    Some("<not shown: union type>".to_owned())
                } else {
                    let num_bytes = cell_bytes(cell.as_ref());
                    (MAX_CELL_BYTES < num_bytes).then(|| {
                        format!("<not shown: {}>", re_format::format_bytes(num_bytes as f64))
                    })
                };
                Ok(match placeholder {
                    Some(placeholder) => (
                        Field::new(name, DataType::Utf8, true),
                        Arc::new(StringArray::from(vec![placeholder; cell.len()])) as ArrayRef,
                    ),
                    None => (field.as_ref().clone().with_name(name), cell.clone()),
                })
            })
            .collect::<Result<Vec<_>, CatalogToolError>>()?
            .into_iter()
            .unzip();
    let row = RecordBatch::try_new_with_options(
        Arc::new(Schema::new_with_metadata(
            fields,
            row.schema().metadata().clone(),
        )),
        cells,
        &arrow::array::RecordBatchOptions::new().with_row_count(Some(row.num_rows())),
    )
    .map_err(|source| CatalogToolError::EncodeJson { source })?;

    let mut writer = arrow::json::WriterBuilder::new()
        .with_explicit_nulls(true)
        .build::<_, arrow::json::writer::JsonArray>(Vec::new());
    writer
        .write(&row)
        .and_then(|()| writer.finish())
        .map_err(|source| CatalogToolError::EncodeJson { source })?;
    let objects: Vec<serde_json::Map<String, Value>> = serde_json::from_slice(&writer.into_inner())
        .map_err(|source| CatalogToolError::DecodeJson { source })?;
    let mut object = objects.into_iter().next().unwrap_or_default();
    Ok((0..row.num_columns())
        .map(|position| object.remove(&position.to_string()).unwrap_or(Value::Null))
        .collect())
}

/// How many bytes the values of the one-row `cell` take.
fn cell_bytes(cell: &dyn arrow::array::Array) -> usize {
    // String and binary views keep long values in shared buffers that the view arrays do not
    // count per row, so they are measured as plain strings and binaries.
    let plain = match cell.data_type() {
        DataType::Utf8View => arrow::compute::cast(cell, &DataType::LargeUtf8).ok(),
        DataType::BinaryView => arrow::compute::cast(cell, &DataType::LargeBinary).ok(),
        _ => None,
    };
    let cell = plain.as_deref().unwrap_or(cell);
    // Measured on a deep copy: a shallow slice of a list reports every row's values.
    let copy = re_arrow_util::deep_slice_array_erased(
        cell,
        re_span::Span {
            start: 0,
            len: cell.len(),
        },
    );
    copy.to_data()
        .get_slice_memory_size()
        .unwrap_or_else(|_| copy.get_array_memory_size())
}

fn contains_union(datatype: &DataType) -> bool {
    match datatype {
        DataType::Union(..) => true,
        DataType::List(field)
        | DataType::LargeList(field)
        | DataType::ListView(field)
        | DataType::LargeListView(field)
        | DataType::FixedSizeList(field, _)
        | DataType::Map(field, _) => contains_union(field.data_type()),
        DataType::Struct(fields) => fields.iter().any(|field| contains_union(field.data_type())),
        DataType::Dictionary(_, value) => contains_union(value),
        DataType::RunEndEncoded(_, value) => contains_union(value.data_type()),
        _ => false,
    }
}

/// Cut long strings and arrays in `value`, so one image does not fill the agent's context.
fn shorten(value: &mut Value) {
    match value {
        Value::String(string) => {
            let num_chars = string.chars().count();
            if MAX_STRING_CHARS < num_chars {
                let head: String = string.chars().take(MAX_STRING_CHARS).collect();
                *string = format!("{head}… ({num_chars} chars)");
            }
        }
        Value::Array(items) => {
            let num_items = items.len();
            if MAX_ARRAY_ITEMS < num_items {
                items.truncate(MAX_ARRAY_ITEMS);
                items.push(json!(format!("… ({num_items} items)")));
            }
            items.iter_mut().for_each(shorten);
        }
        Value::Object(fields) => fields.values_mut().for_each(shorten),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_long_strings_and_arrays_only() {
        let mut value = json!({
            "short": "abc",
            "long": "x".repeat(MAX_STRING_CHARS + 1),
            "pixels": vec![0; MAX_ARRAY_ITEMS + 4],
            "few": [1, 2],
        });
        shorten(&mut value);
        assert_eq!(value["short"], "abc");
        assert!(
            value["long"]
                .as_str()
                .is_some_and(|s| s.ends_with("(201 chars)"))
        );
        assert_eq!(
            value["pixels"].as_array().map(Vec::len),
            Some(MAX_ARRAY_ITEMS + 1)
        );
        assert_eq!(value["few"], json!([1, 2]));
    }

    #[tokio::test]
    async fn only_queries_run() {
        let ctx = session().expect("session");
        let table = RecordBatch::try_from_iter([(
            "x",
            Arc::new(arrow::array::Int32Array::from(vec![1, 2])) as ArrayRef,
        )])
        .expect("batch");
        ctx.register_batch("t", table).expect("register");
        for query in [
            "INSERT INTO t VALUES (3)",
            "UPDATE t SET x = 0",
            "DELETE FROM t",
            "CREATE VIEW v AS SELECT 1",
            "COPY (SELECT 1) TO '/tmp/out.csv'",
            "SET datafusion.execution.batch_size = 1",
        ] {
            let err = collect_rows(&ctx, query, 10).await.expect_err(query);
            assert!(
                matches!(err, CatalogToolError::InvalidQuery { .. })
                    && err.to_string().contains("not supported"),
                "{query}: {err}"
            );
        }
        let unchanged = collect_rows(&ctx, "SELECT x FROM t", 10)
            .await
            .expect("a query");
        assert_eq!(unchanged["rows"], json!([[1], [2]]));
        let ok = collect_rows(&ctx, "SELECT 1 AS one, NULL AS nothing", 10)
            .await
            .expect("a query");
        assert_eq!(ok["rows"], json!([[1, null]]));
    }

    #[tokio::test]
    async fn columns_of_the_same_name_keep_their_values() {
        let ctx = session().expect("session");
        let joined = collect_rows(
            &ctx,
            "SELECT a.x, b.x FROM (SELECT 1 AS x) a CROSS JOIN (SELECT 2 AS x) b",
            10,
        )
        .await
        .expect("a join");
        assert_eq!(joined, json!({ "columns": ["x", "x"], "rows": [[1, 2]] }));
    }

    #[tokio::test]
    async fn large_cells_are_not_encoded() {
        let ctx = session().expect("session");
        let result = collect_rows(
            &ctx,
            &format!(
                "SELECT 1 AS id, repeat('x', {}) AS blob",
                MAX_CELL_BYTES + 1
            ),
            10,
        )
        .await
        .expect("rows");
        let blob = result["rows"][0][1].as_str().expect("a placeholder");
        assert!(blob.starts_with("<not shown: "), "{blob}");
        assert_eq!(result["rows"][0][0], 1);
    }

    #[test]
    fn a_small_list_cell_is_encoded_even_in_a_large_batch() {
        let values: Vec<Option<Vec<Option<i32>>>> =
            (0..100_000).map(|i| Some(vec![Some(i), Some(i)])).collect();
        let lists = arrow::array::ListArray::from_iter_primitive::<arrow::datatypes::Int32Type, _, _>(
            values,
        );
        let batch =
            RecordBatch::try_from_iter([("list", Arc::new(lists) as ArrayRef)]).expect("batch");
        assert_eq!(row_json(&batch.slice(7, 1)).expect("row"), [json!([7, 7])]);
    }

    #[tokio::test]
    async fn wide_rows_keep_every_cell() {
        let ctx = session().expect("session");
        let select: Vec<String> = (0..MAX_ARRAY_ITEMS + 4)
            .map(|i| format!("{i} AS c{i}"))
            .collect();
        let result = collect_rows(&ctx, &format!("SELECT {}", select.join(", ")), 10)
            .await
            .expect("a row");
        let expected: Vec<usize> = (0..MAX_ARRAY_ITEMS + 4).collect();
        assert_eq!(result["rows"], json!([expected]));
    }

    #[tokio::test]
    async fn too_many_columns_is_an_error() {
        let ctx = session().expect("session");
        let long_name = "c".repeat(100);
        let select: Vec<String> = (0..1000)
            .map(|i| format!("{i} AS {long_name}{i}"))
            .collect();
        let err = collect_rows(&ctx, &format!("SELECT {}", select.join(", ")), 10)
            .await
            .expect_err("too many columns");
        assert!(
            matches!(err, CatalogToolError::TooManyColumns { num_columns: 1000 }),
            "{err}"
        );
    }

    #[tokio::test]
    async fn responses_stay_within_the_byte_budget() {
        let ctx = session().expect("session");
        let result = collect_rows(
            &ctx,
            "SELECT value, repeat('x', 100) AS text FROM generate_series(1, 10000)",
            MAX_ROWS,
        )
        .await
        .expect("rows");
        assert!(result["truncated"].is_string(), "{result}");
        assert!(result.to_string().len() <= MAX_RESULT_BYTES);
    }

    #[tokio::test]
    async fn max_rows_is_capped() {
        let ctx = session().expect("session");
        let result = collect_rows(&ctx, "SELECT * FROM generate_series(1, 2000)", 5000)
            .await
            .expect("rows");
        assert_eq!(result["rows"].as_array().map(Vec::len), Some(MAX_ROWS));
        let truncated = result["truncated"].as_str().expect("cut");
        assert!(!truncated.contains("raise `max_rows`"), "{truncated}");
    }

    #[test]
    fn long_string_views_are_measured_in_full() {
        let long = "x".repeat(MAX_CELL_BYTES + 1);
        let views = arrow::array::StringViewArray::from(vec![long.as_str(), "short"]);
        assert!(MAX_CELL_BYTES < cell_bytes(&views.slice(0, 1)));
        assert!(cell_bytes(&views.slice(1, 1)) < 100);
    }

    #[tokio::test]
    async fn says_why_rows_were_cut() {
        let ctx = session().expect("session");
        let by_count = collect_rows(&ctx, "SELECT * FROM generate_series(1, 10)", 3)
            .await
            .expect("rows");
        assert_eq!(by_count["rows"].as_array().map(Vec::len), Some(3));
        assert!(
            by_count["truncated"]
                .as_str()
                .is_some_and(|s| s.contains("max_rows"))
        );

        let by_size = collect_rows(
            &ctx,
            &format!(
                "SELECT repeat(CAST(value AS VARCHAR), {MAX_STRING_CHARS}) \
                 FROM generate_series(1, 1000)"
            ),
            MAX_ROWS,
        )
        .await
        .expect("rows");
        assert!(
            by_size["truncated"]
                .as_str()
                .is_some_and(|s| s.contains("fit in the response")),
            "{by_size}"
        );
    }
}
