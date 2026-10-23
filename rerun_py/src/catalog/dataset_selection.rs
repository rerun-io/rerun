use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::datatypes::Schema as ArrowSchema;
use arrow::pyarrow::PyArrowType;
use datafusion::catalog::TableProvider;
use datafusion::datasource::empty::EmptyTable;
use itertools::Itertools as _;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyAnyMethods as _;
use pyo3::{Bound, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods};

use re_catalog_client::DatasetSelection;
use re_chunk_store::{QueryExpression, SparseFillStrategy, TimeInt, ViewContentsSelector};
use re_datafusion::DataframeQueryTableProvider;
use re_log_types::EntityPathFilter;
use re_sorbet::{ColumnDescriptor, SorbetColumnDescriptors};
use re_types_core::SegmentId;

use crate::catalog::{
    IndexValuesLike, PyDatasetEntryInternal, PySchemaInternal, PyTableProviderAdapterInternal,
    to_py_err,
};
#[cfg(feature = "perf_telemetry")]
use crate::trace_context::extract_trace_context_from_contextvar;
use crate::trace_context::read_trace_context_from_python;
use crate::utils::wait_for_future;

/// A selection of a dataset with optional segment and content filters applied lazily.
//TODO(RR-3157): add the ability to filter on components, not just entity paths
#[pyclass(
    name = "DatasetSelectionInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyDatasetSelectionInternal {
    dataset: Py<PyDatasetEntryInternal>,

    /// Invariant: `selection.dataset_id()` is the ID of `dataset`.
    // TODO(RR-3158): the API allows using a datafusion dataframe to specify segments. If actual
    // workflow/performance indicates that it would be useful, we should consider keeping the
    // logical plan and lazily execute it instead of materializing the segment IDs.
    selection: DatasetSelection,
}

impl PyDatasetSelectionInternal {
    /// `selection` must select from `dataset`.
    pub fn new(dataset: Py<PyDatasetEntryInternal>, selection: DatasetSelection) -> Self {
        Self { dataset, selection }
    }

    /// Filter schema columns based on content filters.
    fn filter_schema(&self, schema: SorbetColumnDescriptors) -> SorbetColumnDescriptors {
        // Filter columns: keep non-component columns (row_id, index) and matching component columns
        let filtered_columns: Vec<ColumnDescriptor> = schema
            .into_iter()
            .filter(|col| {
                match col {
                    ColumnDescriptor::Component(comp) => {
                        self.selection.selects_entity(&comp.entity_path)
                    }
                    // Keep row_id and index columns
                    ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => true,
                }
            })
            .collect();

        SorbetColumnDescriptors {
            columns: filtered_columns,
        }
    }
}

/// Parse `filter_contents()` expressions into a single content filter.
///
/// Each expression may hold several whitespace-separated rules, and parsing ignores errors.
/// An empty list yields a filter without rules, which selects nothing.
pub fn parse_content_filter(exprs: &[String]) -> EntityPathFilter {
    EntityPathFilter::parse_forgiving(exprs.join(" "))
}

#[pymethods]
impl PyDatasetSelectionInternal {
    /// Return the underlying dataset entry.
    #[getter]
    fn dataset(&self, py: Python<'_>) -> Py<PyDatasetEntryInternal> {
        self.dataset.clone_ref(py)
    }

    /// Return the number of segments in the filter (None if no filter).
    ///
    /// This is an internal only function.
    #[getter]
    fn filtered_segment_ids(&self) -> Option<BTreeSet<String>> {
        self.selection
            .segment_filter()
            .map(|ids| ids.iter().map(|id| id.to_string()).collect())
    }

    /// Return the content filter expressions, one per `filter_contents()` call.
    ///
    /// `None` when there is no content filter, never `Some([])`.
    #[getter]
    fn content_filters(&self) -> Option<Vec<String>> {
        let filters = self.selection.content_filters();

        // Note: this logic is kept here in the ffi glue instead of in re_catalog_client because of
        // the denormalized return type (`Some([])` never happens).
        (!filters.is_empty()).then(|| {
            filters
                .iter()
                .map(|filter| {
                    filter
                        .iter_unresolved_expressions_without_properties()
                        .join(" ")
                })
                .collect()
        })
    }

    /// Return the schema of the data contained in this selection.
    fn schema(self_: PyRef<'_, Self>) -> PyResult<PySchemaInternal> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetSelection.schema").entered();
        let dataset = self_.dataset.borrow(py);
        let PySchemaInternal {
            columns: base_columns,
            metadata,
        } = PySchemaInternal::from_arrow_schema(PyDatasetEntryInternal::fetch_arrow_schema(
            &dataset,
        )?)?;

        // Apply content filters
        let filtered_columns = self_.filter_schema(base_columns);
        Ok(PySchemaInternal {
            columns: filtered_columns,
            metadata,
        })
    }

    /// Return the Arrow schema of the data contained in this selection.
    fn arrow_schema(self_: PyRef<'_, Self>) -> PyResult<PyArrowType<ArrowSchema>> {
        let _span =
            read_trace_context_from_python(self_.py(), "DatasetSelection.arrow_schema").entered();
        Ok(Self::schema(self_)?.into_arrow_schema().into())
    }

    /// Returns a list of segment IDs for this selection (filtered if segment filter is set).
    fn segment_ids(self_: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetSelection.segment_ids").entered();
        let connection = self_
            .dataset
            .borrow(py)
            .client()
            .borrow(py)
            .connection()
            .clone();

        connection.get_dataset_segment_ids(py, self_.selection.clone())
    }

    /// Returns a new DatasetSelection filtered to the given segment IDs.
    ///
    /// Parameters
    /// ----------
    /// segment_ids : list[str]
    ///     A list of segment ID strings.
    fn filter_segments(self_: PyRef<'_, Self>, segment_ids: Vec<String>) -> PyResult<Py<Self>> {
        let py = self_.py();
        let _span =
            read_trace_context_from_python(py, "DatasetSelection.filter_segments").entered();

        Py::new(
            py,
            Self::new(
                self_.dataset.clone_ref(py),
                self_
                    .selection
                    .filter_segments(segment_ids.into_iter().map(SegmentId::from)),
            ),
        )
    }

    /// Returns a new DatasetSelection filtered to the given entity paths.
    ///
    /// Parameters
    /// ----------
    /// exprs : list[str]
    ///     Entity path expressions like "/points/**", "-/text/**".
    fn filter_contents(self_: PyRef<'_, Self>, exprs: Vec<String>) -> PyResult<Py<Self>> {
        let py = self_.py();
        let _span =
            read_trace_context_from_python(py, "DatasetSelection.filter_contents").entered();

        Py::new(
            py,
            Self::new(
                self_.dataset.clone_ref(py),
                self_
                    .selection
                    .filter_contents(parse_content_filter(&exprs)),
            ),
        )
    }

    /// Create a reader over this DatasetSelection.
    ///
    /// Returns a DataFusion DataFrame.
    ///
    /// Parameters
    /// ----------
    /// index : str | None
    ///     The index (timeline) to use for the query.
    /// include_semantically_empty_columns : bool
    ///     Whether to include columns that are semantically empty.
    /// include_tombstone_columns : bool
    ///     Whether to include tombstone columns.
    /// fill_latest_at : bool
    ///     Whether to fill null values with the latest valid data.
    /// using_index_values : IndexValuesLike | None
    ///     If provided, the specific index values to sample from all segments.
    #[expect(clippy::fn_params_excessive_bools)]
    #[pyo3(signature = (
        *,
        index,
        include_semantically_empty_columns = false,
        include_tombstone_columns = false,
        fill_latest_at = false,
        using_index_values = None,
    ))]
    fn reader<'py>(
        self_: PyRef<'py, Self>,
        index: Option<String>,
        include_semantically_empty_columns: bool,
        include_tombstone_columns: bool,
        fill_latest_at: bool,
        using_index_values: Option<BTreeMap<String, IndexValuesLike<'_>>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetSelection.reader").entered();

        // Convert IndexValuesLike to BTreeSet<TimeInt>
        let using_index_values = using_index_values
            .map(|values_map| {
                values_map
                    .into_iter()
                    .map(|(k, v)| v.to_index_values().map(|v| (SegmentId::from(k), v)))
                    .try_collect()
            })
            .transpose()?;

        // Build table provider with query parameters
        let provider = build_dataframe_query_table_provider(
            py,
            &self_.dataset,
            &self_.selection,
            index,
            include_semantically_empty_columns,
            include_tombstone_columns,
            fill_latest_at,
            using_index_values,
        )?;

        let table = PyTableProviderAdapterInternal::new(provider);

        let dataset = self_.dataset.borrow(py);
        let client = dataset.client().borrow(py);
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);

        drop(client);
        drop(dataset);

        ctx.call_method1("read_table", (table,))
    }
}

/// Whether `schema` has an index column named `index`.
fn has_index(schema: &ArrowSchema, index: &str) -> bool {
    schema.field_with_name(index).is_ok_and(|field| {
        field
            .metadata()
            .get(re_sorbet::metadata::RERUN_KIND)
            .is_some_and(|value| value == "index")
    })
}

/// Build a `ViewContentsSelector` from the selection's content filters.
fn build_view_contents(schema: &ArrowSchema, selection: &DatasetSelection) -> ViewContentsSelector {
    // Like `DatasetEntry.reader()`, dataframe queries exclude properties unless a content filter names them.
    let selection = if selection.content_filters().is_empty() {
        selection.filter_contents(EntityPathFilter::all())
    } else {
        selection.clone()
    };

    schema
        .fields()
        .iter()
        .filter_map(|field| ColumnDescriptor::try_from_arrow_field(None, field.as_ref()).ok())
        .filter_map(|descriptor| {
            if let ColumnDescriptor::Component(component) = descriptor {
                Some(component)
            } else {
                None
            }
        })
        .filter(|comp| selection.selects_entity(&comp.entity_path))
        .map(|comp| (comp.entity_path.clone(), None))
        .collect()
}

/// Build a table provider for dataframe queries with the given parameters.
#[expect(clippy::fn_params_excessive_bools)]
fn build_dataframe_query_table_provider(
    py: Python<'_>,
    dataset: &Py<PyDatasetEntryInternal>,
    selection: &DatasetSelection,
    index: Option<String>,
    include_semantically_empty_columns: bool,
    include_tombstone_columns: bool,
    fill_latest_at: bool,
    using_index_values: Option<BTreeMap<SegmentId, BTreeSet<TimeInt>>>,
) -> PyResult<Arc<dyn TableProvider + Send>> {
    let dataset_ref = dataset.borrow(py);
    let dataset_id = dataset_ref.entry_id();
    let mut schema = PyDatasetEntryInternal::cached_arrow_schema(&dataset_ref)?;

    // Error if the queried index is unknown.
    if let Some(index) = index.as_deref() {
        // Rejecting the index here sends no request, so nothing would ever advance the revision
        // watermark: an index added by another client would stay invisible on this handle. Refetch
        // once before giving up.
        if !has_index(&schema, index) {
            schema = PyDatasetEntryInternal::fetch_arrow_schema(&dataset_ref)?;
        }

        if !has_index(&schema, index) {
            return Err(PyValueError::new_err(format!(
                "Index '{index}' does not exist in the dataset."
            )));
        }
    }

    let client_ref = dataset_ref.client().borrow(py);
    let connection = client_ref.connection().clone();
    let object_store_auth = client_ref.object_store_auth().clone();
    drop(client_ref);
    drop(dataset_ref);

    let view_contents = build_view_contents(&schema, selection);
    let segment_ids: Vec<&str> = selection
        .segment_filter()
        .map(|ids| ids.iter().map(SegmentId::as_str).collect())
        .unwrap_or_default();

    let static_only = index.is_none();

    let query_expression = QueryExpression {
        view_contents: Some(view_contents),
        include_semantically_empty_columns,
        include_tombstone_columns,
        include_static_columns: if static_only {
            re_chunk_store::StaticColumnSelection::StaticOnly
        } else {
            re_chunk_store::StaticColumnSelection::Both
        },
        filtered_index: index
            .map(|index| {
                re_chunk::TimelineName::try_new(index)
                    .map_err(|err| PyValueError::new_err(err.to_string()))
            })
            .transpose()?,
        filtered_index_range: None,
        filtered_index_values: None,
        using_index_values: None,
        filtered_is_not_null: None,
        sparse_fill_strategy: if fill_latest_at {
            SparseFillStrategy::LatestAtGlobal
        } else {
            SparseFillStrategy::None
        },
        selection: None,
    };

    // Capture trace context to propagate into async query execution
    cfg_select! {
        all(feature = "perf_telemetry", not(target_arch = "wasm32")) => {
            let trace_headers = extract_trace_context_from_contextvar(py);
            let trace_headers_opt = if trace_headers.traceparent.is_empty() {
                None
            } else {
                Some(trace_headers)
            };
        }
        _ => {
            let trace_headers_opt = None;
        }
    }

    let index_values = using_index_values.map(Arc::new);
    // Reuse the already-fetched schema so the provider skips its own `GetDatasetSchema` RPC.
    let arrow_schema = Some(schema);

    // Bind any active `query_metrics()` collectors to this query at plan
    // construction time. Empty when no scope is open; the read traverses the
    // Python `ContextVar` so only collectors from this thread/task are
    // captured. Concurrent or unrelated queries elsewhere in the process are
    // not affected.
    let metrics_collectors = crate::query_metrics::active_metrics_collectors(py);

    wait_for_future(py, async move {
        DataframeQueryTableProvider::new(
            connection.inner().connection().clone(),
            dataset_id,
            &query_expression,
            &segment_ids,
            index_values,
            arrow_schema,
            #[cfg(not(target_arch = "wasm32"))]
            trace_headers_opt,
            metrics_collectors,
            #[cfg(not(target_arch = "wasm32"))]
            object_store_auth,
        )
        .await
    })
    .map(|provider| {
        // The query request would read every segment for an empty selection; keep the
        // provider's schema and validation, but return no rows.
        if selection.selects_no_segments() {
            Arc::new(EmptyTable::new(provider.schema())) as Arc<dyn TableProvider + Send>
        } else {
            Arc::new(provider) as Arc<dyn TableProvider + Send>
        }
    })
    .map_err(to_py_err)
}
