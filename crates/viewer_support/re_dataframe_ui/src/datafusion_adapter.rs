use std::mem;
use std::sync::Arc;

use ahash::HashMap;
use arrow::datatypes::{DataType, SchemaRef};
use arrow::error::ArrowError;
use crossbeam::channel::{Receiver, TryRecvError};
use datafusion::common::{DataFusionError, SchemaError, TableReference};
use datafusion::execution::SendableRecordBatchStream;
use datafusion::execution::SessionState;
use datafusion::functions::expr_fn::concat;
use datafusion::logical_expr::{Expr, LogicalPlanBuilder, binary_expr, col as datafusion_col, lit};
use datafusion::prelude::DataFrame;
use datafusion::prelude::{SessionContext, cast, encode};
use datafusion::sql::sqlparser::parser::ParserError;
use futures::{StreamExt as _, TryStreamExt as _};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_async::AsyncRuntimeHandle;
use re_log::{error, warn};
use re_log_types::Timestamp;
use re_mutex::Mutex;
use re_quota_channel::send_crossbeam;
use re_sdk_types::blueprint::components::ColumnName;
use re_sorbet::{BatchType, SorbetBatch, SorbetSchema};

use crate::cards_view::FlagChangeEvent;
use crate::column_sorting::SortBy;
use crate::filters::filter_expression;
use crate::table_selection::TableSelectionState;

/// Information required to generate a segment link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SegmentLinksSpec {
    /// Name of the column to generate.
    pub column_name: ColumnName,

    /// Name of the existing column containing the segment id.
    pub segment_id_column_name: ColumnName,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,

    /// The id of the dataset to use for the links.
    pub dataset_id: re_log_types::EntryId,
}

/// Information required to generate an entry link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryLinksSpec {
    /// Name of the column to generate.
    pub column_name: ColumnName,

    /// Name of the existing column containing the entry id.
    pub entry_id_column_name: ColumnName,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,
}

/// Make sure we escape column names correctly for datafusion.
///
/// Background: even when round-tripping column names from the very schema that datafusion returns,
/// it can happen that column names have the "wrong" case and must be escaped. See this issue:
/// <https://github.com/apache/datafusion/issues/15922>
///
/// This function is named such as to replace the datafusion's `col` function, so we do the right
/// thing even if we forget about it.
fn col(name: &str) -> datafusion::logical_expr::Expr {
    datafusion_col(format!("{name:?}"))
}

/// The subset of `TableBlueprint` that is actually handled by datafusion.
///
/// In general, there are aspects of a table blueprint that are handled by the UI in an immediate
/// mode fashion (e.g. is a column visible?), and other aspects that are handled by datafusion (e.g.
/// sorting). This struct is for the latter.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct DataFusionQueryData {
    pub sort_by: Option<SortBy>,
    pub segment_links: Option<SegmentLinksSpec>,
    pub entry_links: Option<EntryLinksSpec>,
    pub prefilter: Option<datafusion::prelude::Expr>,

    /// SQL expressions from the table blueprint that select the shown rows.
    pub filters: Vec<String>,
}

/// Result of the async datafusion query process.
#[derive(Debug, Clone)]
pub struct DataFusionQueryResult {
    /// The record batches to display.
    pub sorbet_batches: Vec<SorbetBatch>,

    /// The schema of the record batches.
    pub original_schema: SchemaRef,

    /// The migrated schema of the record batches (useful when the list of batches is empty).
    pub sorbet_schema: re_sorbet::SorbetSchema,

    /// The error of each filter that the query left out, by the filter's SQL.
    pub filter_errors: FilterErrors,

    pub finished: bool,
}

/// Filter errors by the filter's SQL.
pub type FilterErrors = Arc<HashMap<String, String>>;

/// The message of a query or filter error, for showing in the UI.
///
/// Uses the message of the innermost error without the kind of error in front of it, and leaves
/// out lists of every column or candidate function.
pub fn query_error_message(err: &DataFusionError) -> String {
    let message = match err.find_root() {
        DataFusionError::SQL(parser_error, _) => sql_parser_error_message(parser_error),
        DataFusionError::Plan(message)
        | DataFusionError::Execution(message)
        | DataFusionError::NotImplemented(message) => message.clone(),
        DataFusionError::ArrowError(arrow_error, _) => match arrow_error.as_ref() {
            ArrowError::CastError(message)
            | ArrowError::ComputeError(message)
            | ArrowError::InvalidArgumentError(message) => message.clone(),
            arrow_error => arrow_error.to_string(),
        },
        DataFusionError::SchemaError(schema_error, _) => schema_error_message(schema_error),
        root => root.strip_backtrace(),
    };

    // Lists of candidate functions are on lines that start with a tab.
    message
        .lines()
        .filter(|line| !line.starts_with('\t'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A filter is a single line, so a location only needs the column.
fn sql_parser_error_message(err: &ParserError) -> String {
    let message = match err {
        ParserError::TokenizerError(message) | ParserError::ParserError(message) => message.clone(),
        ParserError::RecursionLimitExceeded => return "The filter is nested too deeply".to_owned(),
    };
    message
        .replacen("Expected: ", "Expected ", 1)
        .replace(", found: EOF", ", but the filter ended")
        .replace(", found: ", ", found ")
        .replace(" at Line: 1, Column: ", " at character ")
}

/// `SchemaError::FieldNotFound` lists every column of the table.
fn schema_error_message(err: &SchemaError) -> String {
    match err {
        SchemaError::FieldNotFound {
            field,
            valid_fields,
        } => {
            let name = &field.name;
            let similar = valid_fields
                .iter()
                .find(|column| column.name.eq_ignore_ascii_case(name));
            if let Some(similar) = similar {
                format!(
                    "No column named {name:?}. Column names are case sensitive, did you mean {:?}?",
                    similar.name
                )
            } else {
                format!("No column named {name:?}")
            }
        }
        SchemaError::AmbiguousReference { field } => {
            format!("Column name {:?} is ambiguous", field.name)
        }
        SchemaError::DuplicateQualifiedField { name, .. }
        | SchemaError::DuplicateUnqualifiedField { name } => {
            format!("Duplicate column {name:?}")
        }
    }
}

/// Plan a filter of the filter bar.
///
/// Type checks and constant folding of the filter run here, so a filter that fails them returns
/// an error of its own instead of failing the whole query.
fn plan_filter(
    session_state: &SessionState,
    dataframe: &DataFrame,
    sql: &str,
) -> Result<Expr, DataFusionError> {
    let expr = filter_expression(session_state, dataframe.schema(), sql)?;
    let plan = LogicalPlanBuilder::from(dataframe.logical_plan().clone())
        .filter(expr.clone())?
        .build()?;
    session_state.optimize(&plan)?;
    Ok(expr)
}

impl DataFusionQueryResult {
    /// Resolve a global row index to `(batch_index, row_offset_within_batch)`.
    fn find_row_indices(&self, global_row: u64) -> Option<(usize, usize)> {
        let mut remaining = global_row as usize;
        for (batch_idx, batch) in self.sorbet_batches.iter().enumerate() {
            let num_rows = batch.num_rows();
            if remaining < num_rows {
                return Some((batch_idx, remaining));
            }
            remaining -= num_rows;
        }
        None
    }

    /// Resolve a global row index to a batch reference and the row offset within that batch.
    pub fn find_row_batch(&self, global_row: u64) -> Option<(&SorbetBatch, usize)> {
        let (idx, offset) = self.find_row_indices(global_row)?;
        Some((&self.sorbet_batches[idx], offset))
    }

    /// Mutable variant of [`Self::find_row_batch`].
    pub fn find_row_batch_mut(&mut self, global_row: u64) -> Option<(&mut SorbetBatch, usize)> {
        let (idx, offset) = self.find_row_indices(global_row)?;
        Some((&mut self.sorbet_batches[idx], offset))
    }
}

/// Query state and the context required to execute the corresponding datafusion query.
#[derive(Clone)]
struct DataFusionQuery {
    session_ctx: Arc<SessionContext>,
    table_ref: TableReference,

    query_data: DataFusionQueryData,
}

impl DataFusionQuery {
    fn new(
        session_ctx: Arc<SessionContext>,
        table_ref: TableReference,
        query_data: DataFusionQueryData,
    ) -> Self {
        Self {
            session_ctx,
            table_ref,
            query_data,
        }
    }

    /// Returns the stream of the query, and the errors of the filters it left out.
    async fn batch_stream(
        self,
    ) -> Result<(SendableRecordBatchStream, FilterErrors), DataFusionError> {
        let mut dataframe = self.session_ctx.table(self.table_ref).await?;

        let DataFusionQueryData {
            sort_by,
            segment_links,
            entry_links,
            prefilter,
            filters,
        } = &self.query_data;

        //
        // Segment links
        //

        // Important: the needs to happen first, in case we sort/filter/etc. based on that
        // particular column.
        if let Some(segment_links) = segment_links {
            //TODO(ab): we should get this from `re_uri::DatasetDataUri` instead of hardcoding
            let uri = format!(
                "{}/dataset/{}/data?segment_id=",
                segment_links.origin, segment_links.dataset_id
            );

            dataframe = dataframe.with_column(
                &segment_links.column_name,
                concat(vec![lit(uri), col(&segment_links.segment_id_column_name)]),
            )?;
        }

        //
        // Entry links
        //

        if let Some(entry_links) = entry_links {
            let uri = format!("{}/entry/", entry_links.origin);

            let column = concat(vec![
                lit(uri),
                encode(
                    cast(col(&entry_links.entry_id_column_name), DataType::Binary),
                    lit("hex"),
                ),
            ]);
            dataframe = dataframe.with_column(&entry_links.column_name, column)?;
        }

        //
        // Prefilter
        //

        if let Some(prefilter) = prefilter {
            dataframe = dataframe.filter(prefilter.clone())?;
        }

        //
        // Filters
        //

        let session_state = self.session_ctx.state();
        let mut filter_errors = HashMap::default();
        let filter_exprs = filters
            .iter()
            .filter_map(
                |filter| match plan_filter(&session_state, &dataframe, filter) {
                    Ok(expr) => Some(expr),
                    Err(err) => {
                        filter_errors.insert(filter.clone(), query_error_message(&err));
                        None
                    }
                },
            )
            .collect();
        let filter_expr =
            balanced_binary_exprs(filter_exprs, datafusion::logical_expr::Operator::And);
        if let Some(filter_expr) = filter_expr {
            dataframe = dataframe.filter(filter_expr)?;
        }

        //
        // Sort
        //

        if let Some(sort_by) = sort_by {
            let ascending = sort_by.direction.is_ascending();
            dataframe =
                dataframe.sort(vec![col(&sort_by.column_name).sort(ascending, ascending)])?;
        }

        //
        // Execute the query
        //

        let stream = dataframe.execute_stream().await?;

        Ok((stream, Arc::new(filter_errors)))
    }

    /// Execute the query to produce the data to display.
    ///
    /// Note: the future returned by this function must be `'static`, so it takes `self`. Use
    /// `clone()` as required.
    fn execute_streaming(self, runtime: &AsyncRuntimeHandle) -> Receiver<QueryEvent> {
        let (tx, rx) = re_quota_channel::create_crossbeam_channel(1000);
        runtime.spawn_future(async move {
            match self.batch_stream().await {
                Err(err) => {
                    send_crossbeam(
                        &tx,
                        QueryEvent::Error(DataFusionQueryError {
                            error: err,
                            original_schema: None,
                            filter_errors: FilterErrors::default(),
                        }),
                    )
                    .ok();
                }
                Ok((stream, filter_errors)) => {
                    let schema = stream.schema();

                    let mut sorbet_stream = stream.and_then(|s| {
                        std::future::ready(
                            SorbetBatch::try_from_record_batch(&s, BatchType::Dataframe)
                                .map_err(|err| DataFusionError::External(err.into())),
                        )
                    });

                    let mut sent_schemas = false;
                    let mut sent_error = false;

                    while let Some(frame) = sorbet_stream.next().await {
                        match frame {
                            Ok(batch) => {
                                if !sent_schemas {
                                    let sorbet_schema = batch.sorbet_schema().clone();
                                    let original_schema = Arc::clone(&schema);
                                    if send_crossbeam(
                                        &tx,
                                        QueryEvent::Schema {
                                            original_schema,
                                            sorbet_schema,
                                            filter_errors: Arc::clone(&filter_errors),
                                        },
                                    )
                                    .is_err()
                                    {
                                        return; // Receiver dropped, stop streaming
                                    }
                                    sent_schemas = true;
                                }
                                if send_crossbeam(&tx, QueryEvent::Batch(batch)).is_err() {
                                    return; // Receiver dropped, stop streaming
                                }
                            }
                            Err(err) => {
                                sent_error = true;
                                send_crossbeam(
                                    &tx,
                                    QueryEvent::Error(DataFusionQueryError {
                                        error: err,
                                        original_schema: Some(Arc::clone(&schema)),
                                        filter_errors: Arc::clone(&filter_errors),
                                    }),
                                )
                                .ok();
                            }
                        }
                    }

                    // We got no results, try to derive the sorbet schema from the raw arrow schema
                    if !sent_schemas && !sent_error {
                        let sorbet_schema = SorbetSchema::try_from_raw_arrow_schema(schema.clone());
                        match sorbet_schema {
                            Ok(sorbet_schema) => {
                                send_crossbeam(
                                    &tx,
                                    QueryEvent::Schema {
                                        original_schema: schema,
                                        sorbet_schema,
                                        filter_errors,
                                    },
                                )
                                .ok();
                            }
                            Err(err) => {
                                send_crossbeam(
                                    &tx,
                                    QueryEvent::Error(DataFusionQueryError {
                                        error: DataFusionError::External(err.into()),
                                        original_schema: Some(schema),
                                        filter_errors,
                                    }),
                                )
                                .ok();
                            }
                        }
                    }
                }
            }
        });
        rx
    }
}

/// A query that failed.
#[derive(Debug)]
pub struct DataFusionQueryError {
    pub error: DataFusionError,

    /// The arrow schema of the query, if the query failed after planning.
    pub original_schema: Option<SchemaRef>,

    /// The error of each filter that the query left out, by the filter's SQL.
    pub filter_errors: FilterErrors,
}

/// A event produced during the streaming execution of a datafusion query.
///
/// It's guaranteed that the first event is either [`QueryEvent::Schema`] or [`QueryEvent::Error`].
#[derive(Debug)]
pub enum QueryEvent {
    Schema {
        original_schema: SchemaRef,
        sorbet_schema: re_sorbet::SorbetSchema,
        filter_errors: FilterErrors,
    },
    Batch(SorbetBatch),
    Error(DataFusionQueryError),
}

impl PartialEq for DataFusionQuery {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            session_ctx,
            table_ref,
            query_data,
        } = self;

        Arc::ptr_eq(session_ctx, &other.session_ctx)
            && table_ref == &other.table_ref
            && query_data == &other.query_data
    }
}

/// Helper struct to manage the datafusion async query and the resulting `SorbetBatch`.
#[derive(Clone)]
pub struct DataFusionAdapter {
    id: egui::Id,

    /// The query used to produce the dataframe.
    query: DataFusionQuery,

    // Used to have something to display while the new dataframe is being queried.
    pub last_query_results: Option<Result<DataFusionQueryResult, Arc<DataFusionQueryError>>>,

    // TODO(ab, lucasmerlin): this `Mutex` is only needed because of the `Clone` bound in egui
    // so we should clean that up if the bound is lifted.
    pub rx: Arc<Mutex<Receiver<QueryEvent>>>,

    pub results: Option<Result<DataFusionQueryResult, Arc<DataFusionQueryError>>>,

    pub queried_at: Timestamp,
}

impl DataFusionAdapter {
    pub fn clear_state(egui_ctx: &egui::Context, id: egui::Id) {
        egui_ctx.data_mut(|data| {
            data.remove::<Self>(id);
        });
    }

    /// Retrieve the state from egui's memory or create a new one if it doesn't exist.
    pub fn get(
        runtime: &AsyncRuntimeHandle,
        ui: &egui::Ui,
        session_ctx: &Arc<SessionContext>,
        table_ref: TableReference,
        id: egui::Id,
        initial_query_data: DataFusionQueryData,
    ) -> Self {
        let adapter = ui.data(|data| data.get_temp::<Self>(id));

        let mut adapter = adapter.unwrap_or_else(|| {
            let query =
                DataFusionQuery::new(Arc::clone(session_ctx), table_ref, initial_query_data);

            let rx = query.clone().execute_streaming(runtime);

            let table_state = Self {
                id,
                rx: Arc::new(Mutex::new(rx)),
                results: None,
                query,
                last_query_results: None,
                queried_at: Timestamp::now(),
            };

            ui.data_mut(|data| {
                data.insert_temp(id, table_state.clone());
            });

            table_state
        });

        {
            let rx = adapter.rx.lock();
            let mut changed = false;
            loop {
                match rx.try_recv() {
                    Ok(QueryEvent::Schema {
                        sorbet_schema,
                        original_schema,
                        filter_errors,
                    }) => {
                        adapter.results = Some(Ok(DataFusionQueryResult {
                            original_schema,
                            sorbet_schema,
                            filter_errors,
                            sorbet_batches: vec![],
                            finished: false,
                        }));
                        changed = true;
                    }
                    Ok(QueryEvent::Batch(batch)) => match &mut adapter.results {
                        Some(Ok(data)) => {
                            data.sorbet_batches.push(batch);
                            changed = true;

                            // We received some data, so stop showing any previous results.
                            adapter.last_query_results = None;
                        }
                        Some(Err(err)) => {
                            warn!("Received data after receiving an error: {}", err.error);
                        }
                        None => {
                            error!("Received data before receiving schema");
                        }
                    },
                    Ok(QueryEvent::Error(err)) => {
                        adapter.results = Some(Err(Arc::new(err)));
                        changed = true;
                    }
                    Err(TryRecvError::Empty) => {
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        if let Some(Ok(data)) = &mut adapter.results {
                            data.finished = true;
                            changed = true;
                        }
                        break;
                    }
                }
            }

            if changed {
                ui.data_mut(|data| {
                    data.insert_temp(adapter.id, adapter.clone());
                });
            }
        }

        adapter
    }

    pub fn query_data(&self) -> &DataFusionQueryData {
        &self.query.query_data
    }

    /// Update the query and save the state to egui's memory.
    ///
    /// If the query has changed (e.g. because the ui mutated it), it is executed to produce a new
    /// dataframe.
    pub fn update_query(
        mut self,
        runtime: &AsyncRuntimeHandle,
        ui: &egui::Ui,
        new_query_data: DataFusionQueryData,
    ) {
        // retrigger a new datafusion query if required.
        if self.query.query_data != new_query_data {
            self.query.query_data = new_query_data;

            self.last_query_results = mem::take(&mut self.results);

            if let Some(Ok(results)) = &mut self.last_query_results {
                results.finished = true;
            }

            let rx = self.query.clone().execute_streaming(runtime);

            self.rx = Arc::new(Mutex::new(rx));

            TableSelectionState::clear(ui.ctx(), self.id);
        }

        ui.data_mut(|data| {
            data.insert_temp(self.id, self);
        });
    }

    /// Apply flag changes to the in-memory query results.
    ///
    /// Note that this only manipulates in-memory state.
    /// Sending this to wherever we got the data from has to happen separately.
    ///
    /// Each edited column must already exist as a boolean column in the Sorbet schema.
    /// Does nothing otherwise.
    pub fn apply_flag_changes(&mut self, ui: &egui::Ui, changes: &[FlagChangeEvent]) {
        let Some(Ok(results)) = &mut self.results else {
            return;
        };

        update_flag_columns(results, changes);

        ui.data_mut(|data| {
            data.insert_temp(self.id, self.clone());
        });
    }
}

/// Update existing flag columns with the given changes.
///
/// Since Arrow arrays are immutable, we must rebuild the entire column even for single-cell changes.
fn update_flag_columns(results: &mut DataFusionQueryResult, changes: &[FlagChangeEvent]) {
    use arrow::array::{Array as _, BooleanArray};

    for change in changes {
        let Some(col_idx) = results
            .original_schema
            .fields()
            .iter()
            .position(|field| field.name() == change.physical_column.as_str())
        else {
            re_log::warn_once!(
                "Flag column {:?} is missing from the original schema",
                change.physical_column
            );
            continue;
        };
        let Some((batch, row_offset)) = results.find_row_batch_mut(change.row) else {
            continue;
        };

        let Some(old_col) = batch.column(col_idx).downcast_array_ref::<BooleanArray>() else {
            re_log::warn_once!("Flag column at index {col_idx} is not a boolean column");
            break;
        };

        let new_col: BooleanArray = (0..batch.num_rows())
            .map(|i| {
                if i == row_offset {
                    Some(change.new_value)
                } else if old_col.is_null(i) {
                    None
                } else {
                    Some(old_col.value(i))
                }
            })
            .collect();

        if let Some(new_batch) = batch.with_replaced_column(col_idx, std::sync::Arc::new(new_col)) {
            *batch = new_batch;
        }
    }
}

/// Creates a _balanced_ chain of binary expressions.
fn balanced_binary_exprs(
    mut exprs: Vec<datafusion::logical_expr::Expr>,
    op: datafusion::logical_expr::Operator,
) -> Option<datafusion::logical_expr::Expr> {
    while exprs.len() > 1 {
        let mut exprs_next = Vec::with_capacity(exprs.len() / 2 + 1);
        let mut exprs_prev = exprs.into_iter();

        while let Some(left) = exprs_prev.next() {
            if let Some(right) = exprs_prev.next() {
                exprs_next.push(binary_expr(left, op, right));
            } else {
                exprs_next.push(left);
            }
        }

        exprs = exprs_next;
    }

    exprs.into_iter().next()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{Int64Array, RecordBatch};
    use datafusion::prelude::SessionContext;
    use re_log_types::EntryId;

    use super::{DataFusionQuery, DataFusionQueryData, EntryLinksSpec, SegmentLinksSpec};
    use crate::column_sorting::SortBy;

    /// The query leaves out filters that fail to plan and returns their error by SQL.
    /// SQL that doesn't parse returns the parser message, and an unknown column returns a short
    /// message that names a column differing only in case.
    #[tokio::test]
    async fn invalid_filters_return_errors() {
        let session_ctx = Arc::new(SessionContext::new());
        let batch = RecordBatch::try_from_iter([
            ("a", Arc::new(Int64Array::from(vec![1, 2, 3])) as _),
            ("B", Arc::new(Int64Array::from(vec![1, 2, 3])) as _),
        ])
        .unwrap();
        session_ctx.register_batch("table", batch).unwrap();

        let valid = r#""a" > 1"#;
        let unknown_column = r#""missing" > 1"#;
        let wrong_case = "b > 1";
        let syntax_error = r#""a" >"#;

        let query = DataFusionQuery::new(
            session_ctx,
            "table".into(),
            DataFusionQueryData {
                filters: vec![
                    valid.to_owned(),
                    unknown_column.to_owned(),
                    wrong_case.to_owned(),
                    syntax_error.to_owned(),
                ],
                ..Default::default()
            },
        );
        let (_stream, filter_errors) = query.batch_stream().await.unwrap();

        assert_eq!(filter_errors.len(), 3, "{filter_errors:?}");
        assert_eq!(
            filter_errors[unknown_column],
            r#"No column named "missing""#
        );
        assert_eq!(
            filter_errors[wrong_case],
            r#"No column named "b". Column names are case sensitive, did you mean "B"?"#
        );
        assert!(
            !filter_errors[syntax_error].contains("ParserError"),
            "{filter_errors:?}"
        );
    }

    /// Filters that parse but fail type checks, constant folding or use aggregate functions are
    /// left out with an error of their own, and the query with the other filters still runs.
    #[tokio::test]
    async fn filters_that_fail_planning_return_errors() {
        let session_ctx = Arc::new(SessionContext::new());
        let batch =
            RecordBatch::try_from_iter([("a", Arc::new(Int64Array::from(vec![1, 2, 3])) as _)])
                .unwrap();
        session_ctx.register_batch("table", batch).unwrap();

        let errors = [
            (
                r#""a" + 1"#,
                "The filter produces Int64 values instead of true or false",
            ),
            (
                r#""a" > 'hello'"#,
                "Cannot cast string 'hello' to value of Int64 type",
            ),
            (
                r#""a" LIKE '%x%'"#,
                "There isn't a common type to coerce Int64 and Utf8 in LIKE expression",
            ),
            (
                r#"sum("a") > 1"#,
                "A filter can't use functions that combine rows, such as sum or count",
            ),
            (
                r#"abs("a", 1)"#,
                "Function 'abs' expects 1 arguments but received 2. No function matches the given name and argument types 'abs(Int64, Int64)'. You might need to add explicit type casts.",
            ),
            (r#"(("a" > 1)"#, "Expected ), but the filter ended"),
        ];

        let mut filters = vec![r#""a" > 1"#.to_owned()];
        filters.extend(errors.iter().map(|(filter, _)| (*filter).to_owned()));
        let query = DataFusionQuery::new(
            session_ctx,
            "table".into(),
            DataFusionQueryData {
                filters,
                ..Default::default()
            },
        );
        let (stream, filter_errors) = query.batch_stream().await.unwrap();

        for (filter, error) in errors {
            assert_eq!(filter_errors.get(filter).map(String::as_str), Some(error));
        }
        let batches: Vec<RecordBatch> = futures::TryStreamExt::try_collect(stream).await.unwrap();
        let num_rows: usize = batches.iter().map(RecordBatch::num_rows).sum();
        assert_eq!(num_rows, 2);
    }

    #[test]
    fn query_inputs_change_query_fingerprint() {
        let baseline = DataFusionQueryData::default();

        let mut changed = baseline.clone();
        changed.sort_by = Some(SortBy::ascending("sort".into()));
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed.prefilter = Some(datafusion::prelude::col("prefilter"));
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed
            .filters
            .push(r#""filter" ILIKE '%value%'"#.to_owned());
        assert_ne!(baseline, changed);

        let origin: re_uri::Origin = "rerun+http://127.0.0.1:9876".parse().unwrap();
        let mut changed = baseline.clone();
        changed.segment_links = Some(SegmentLinksSpec {
            column_name: "segment_link".into(),
            segment_id_column_name: "segment_id".into(),
            origin: origin.clone(),
            dataset_id: EntryId::new(),
        });
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed.entry_links = Some(EntryLinksSpec {
            column_name: "entry_link".into(),
            entry_id_column_name: "entry_id".into(),
            origin,
        });
        assert_ne!(baseline, changed);
    }
}
