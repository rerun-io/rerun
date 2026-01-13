use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use arrow::datatypes::Schema as ArrowSchema;
use arrow::pyarrow::PyArrowType;
use datafusion::catalog::TableProvider;
use pyo3::prelude::PyAnyMethods as _;
use pyo3::{Bound, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods};
use re_chunk_store::{QueryExpression, SparseFillStrategy, TimeInt, ViewContentsSelector};
use re_datafusion::DataframeQueryTableProvider;
use re_log_types::{EntityPathFilter, ResolvedEntityPathFilter};
#[cfg(feature = "perf_telemetry")]
use re_perf_telemetry::extract_trace_context_from_contextvar;
use re_sorbet::{ColumnDescriptor, SorbetColumnDescriptors};
use tracing::instrument;

#[cfg(feature = "perf_telemetry")]
use crate::catalog::trace_context::with_trace_span;
use crate::catalog::{
    IndexValuesLike, PyDatasetEntryInternal, PySchemaInternal, PyTableProviderAdapterInternal,
    to_py_err,
};
use crate::utils::wait_for_future;

/// A view over a dataset with optional segment and content filters applied lazily.
#[pyclass(name = "DatasetViewInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq]
pub struct PyDatasetViewInternal {
    dataset: Py<PyDatasetEntryInternal>,

    /// Segment filter: `None` means no filtering, `Some` means filter to these segment IDs
    // TODO(RR-3158): the API allows using a datafusion dataframe to specify segments. If actual
    // workflow/performance indicates that it would be useful, we should consider keeping the
    // logical plan and lazily execute it instead of materializing the segment IDs.
    segment_filter: Option<HashSet<String>>,

    /// Content filters: entity path expressions like "/points/**", "-/text/**". If empty,
    /// everything is included. Mutually exclusive with `column_selectors`.
    content_filters: Vec<String>,

    /// Column selectors: component column selectors like "/entity_path:Component". If Some,
    /// this is used instead of `content_filters` for component-level filtering.
    column_selectors: Option<Vec<String>>,
}

impl PyDatasetViewInternal {
    /// Create a new `DatasetView` from a dataset with optional initial filters.
    pub fn new(
        dataset: Py<PyDatasetEntryInternal>,
        segment_filter: Option<HashSet<String>>,
        content_filters: Option<Vec<String>>,
    ) -> Self {
        Self {
            dataset,
            segment_filter,
            content_filters: content_filters.unwrap_or_default(),
            column_selectors: None,
        }
    }

    /// Create a new `DatasetView` with column selectors for component-level filtering.
    pub fn new_with_column_selectors(
        dataset: Py<PyDatasetEntryInternal>,
        segment_filter: Option<HashSet<String>>,
        column_selectors: Vec<String>,
    ) -> Self {
        Self {
            dataset,
            segment_filter,
            content_filters: Vec::new(),
            column_selectors: Some(column_selectors),
        }
    }

    /// Get the resolved entity path filter from content filter expressions.
    fn resolved_entity_path_filter(&self) -> ResolvedEntityPathFilter {
        if self.content_filters.is_empty() {
            // Accept everything
            EntityPathFilter::parse_forgiving("/**").resolve_without_substitutions()
        } else {
            let expr = self.content_filters.join(" ");
            EntityPathFilter::parse_forgiving(&expr).resolve_without_substitutions()
        }
    }

    /// Filter schema columns based on content filters or column selectors.
    fn filter_schema(&self, schema: SorbetColumnDescriptors) -> PyResult<SorbetColumnDescriptors> {
        // If we have column selectors, use component-level filtering
        if let Some(column_selectors) = &self.column_selectors {
            return self.filter_schema_by_column_selectors(schema, column_selectors);
        }

        // Otherwise use entity path filtering
        if self.content_filters.is_empty() {
            return Ok(schema);
        }

        let filter = self.resolved_entity_path_filter();

        // Filter columns: keep non-component columns (row_id, index) and matching component columns
        let filtered_columns: Vec<ColumnDescriptor> = schema
            .into_iter()
            .filter(|col| {
                match col {
                    ColumnDescriptor::Component(comp) => filter.matches(&comp.entity_path),
                    // Keep row_id and index columns
                    ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => true,
                }
            })
            .collect();

        Ok(SorbetColumnDescriptors {
            columns: filtered_columns,
        })
    }

    /// Filter schema columns based on column selectors.
    fn filter_schema_by_column_selectors(
        &self,
        schema: SorbetColumnDescriptors,
        column_selectors: &[String],
    ) -> PyResult<SorbetColumnDescriptors> {
        use std::str::FromStr;
        use re_sorbet::ComponentColumnSelector;

        // Parse all column selectors - fail fast with clear error
        let selectors: Result<Vec<ComponentColumnSelector>, _> = column_selectors
            .iter()
            .map(|s| ComponentColumnSelector::from_str(s))
            .collect();

        let selectors = selectors.map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Invalid column selector format: {e}. Expected format: 'entity_path:component' (e.g., '/world/points:Position3D')"
            ))
        })?;

        // Filter columns: keep non-component columns and matching component columns
        let filtered_columns: Vec<ColumnDescriptor> = schema
            .into_iter()
            .filter(|col| {
                match col {
                    ColumnDescriptor::Component(comp) => {
                        // Check if this component matches any selector
                        selectors.iter().any(|selector| comp.matches(selector))
                    }
                    // Keep row_id and index columns
                    ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => true,
                }
            })
            .collect();

        Ok(SorbetColumnDescriptors {
            columns: filtered_columns,
        })
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyDatasetViewInternal {
    /// Return the underlying dataset entry.
    #[getter]
    fn dataset(&self, py: Python<'_>) -> Py<PyDatasetEntryInternal> {
        self.dataset.clone_ref(py)
    }

    /// Return the number of segments in the filter (None if no filter).
    ///
    /// This is an internal only function.
    #[getter]
    fn filtered_segment_ids(&self) -> Option<HashSet<String>> {
        self.segment_filter.clone()
    }

    /// Return the content filter expressions.
    #[getter]
    fn content_filters(&self) -> Vec<String> {
        self.content_filters.clone()
    }

    /// Return the schema of the data contained in this view.
    #[instrument(skip_all)]
    fn schema(self_: PyRef<'_, Self>) -> PyResult<PySchemaInternal> {
        let py = self_.py();
        let dataset = self_.dataset.borrow(py);
        let PySchemaInternal {
            columns: base_columns,
            metadata,
        } = PyDatasetEntryInternal::fetch_schema(&dataset)?;

        // Apply content filters
        let filtered_columns = self_.filter_schema(base_columns)?;
        Ok(PySchemaInternal {
            columns: filtered_columns,
            metadata,
        })
    }

    /// Return the Arrow schema of the data contained in this view.
    #[instrument(skip_all)]
    fn arrow_schema(self_: PyRef<'_, Self>) -> PyResult<PyArrowType<ArrowSchema>> {
        Ok(Self::schema(self_)?.into_arrow_schema().into())
    }

    /// Returns a list of segment IDs for this view (filtered if segment filter is set).
    fn segment_ids(self_: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let py = self_.py();
        let dataset = self_.dataset.borrow(py);

        let all_segment_ids = PyDatasetEntryInternal::segment_ids(dataset)?;

        match &self_.segment_filter {
            Some(filter) => Ok(all_segment_ids
                .into_iter()
                .filter(|id| filter.contains(id))
                .collect()),
            None => Ok(all_segment_ids),
        }
    }

    /// Returns a new DatasetView filtered to the given segment IDs.
    ///
    /// Parameters
    /// ----------
    /// segment_ids : list[str]
    ///     A list of segment ID strings.
    fn filter_segments(self_: PyRef<'_, Self>, segment_ids: Vec<String>) -> PyResult<Py<Self>> {
        let py = self_.py();

        // Extract segment IDs from input
        let new_segments: HashSet<String> = segment_ids.into_iter().collect();

        let combined_segments = match &self_.segment_filter {
            Some(existing) => existing.intersection(&new_segments).cloned().collect(),
            None => new_segments,
        };

        Py::new(
            py,
            Self::new(
                self_.dataset.clone_ref(py),
                Some(combined_segments),
                Some(self_.content_filters.clone()),
            ),
        )
    }

    /// Returns a new DatasetView filtered to the given entity paths.
    ///
    /// Parameters
    /// ----------
    /// exprs : list[str]
    ///     Entity path expressions like "/points/**", "-/text/**".
    fn filter_contents(self_: PyRef<'_, Self>, exprs: Vec<String>) -> PyResult<Py<Self>> {
        let py = self_.py();

        // Combine with existing content filters
        let mut combined_filters = self_.content_filters.clone();
        combined_filters.extend(exprs);

        Py::new(
            py,
            Self::new(
                self_.dataset.clone_ref(py),
                self_.segment_filter.clone(),
                Some(combined_filters),
            ),
        )
    }

    /// Returns a new DatasetView filtered to the given component column selectors.
    ///
    /// Parameters
    /// ----------
    /// column_selectors : list[str]
    ///     Component column selectors like "/entity_path:Component".
    fn filter_contents_columns(
        self_: PyRef<'_, Self>,
        column_selectors: Vec<String>,
    ) -> PyResult<Py<Self>> {
        let py = self_.py();

        // Combine with existing column selectors if any
        let combined_selectors = match &self_.column_selectors {
            Some(existing) => {
                let mut combined = existing.clone();
                combined.extend(column_selectors);
                combined
            }
            None => column_selectors,
        };

        Py::new(
            py,
            Self::new_with_column_selectors(
                self_.dataset.clone_ref(py),
                self_.segment_filter.clone(),
                combined_selectors,
            ),
        )
    }

    /// Create a reader over this DatasetView.
    ///
    /// Returns a DataFusion DataFrame.
    ///
    /// Parameters
    /// ----------
    /// index : str | None
    ///     The index (timeline) to use for the view.
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
    #[instrument(skip_all)]
    fn reader<'py>(
        self_: PyRef<'py, Self>,
        index: Option<String>,
        include_semantically_empty_columns: bool,
        include_tombstone_columns: bool,
        fill_latest_at: bool,
        using_index_values: Option<IndexValuesLike<'_>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();

        // Convert IndexValuesLike to BTreeSet<TimeInt>
        let using_index_values = using_index_values
            .map(|v| v.to_index_values())
            .transpose()?;

        // Build table provider with query parameters
        let provider = build_dataframe_query_table_provider(
            py,
            &self_.dataset,
            self_.segment_filter.clone(),
            &self_.content_filters,
            &self_.column_selectors,
            index,
            include_semantically_empty_columns,
            include_tombstone_columns,
            fill_latest_at,
            using_index_values,
        )?;

        let table = PyTableProviderAdapterInternal::new(provider, true);

        #[cfg(feature = "perf_telemetry")]
        {
            with_trace_span!(py, "reader", {
                // Get context and call read_table with the reader
                let dataset = self_.dataset.borrow(py);
                let client = dataset.client().borrow(py);
                let ctx = client.ctx(py)?;
                let ctx = ctx.bind(py);

                drop(client);
                drop(dataset);

                ctx.call_method1("read_table", (table,))
            })
        }
        #[cfg(not(feature = "perf_telemetry"))]
        {
            // Get context and call read_table with the reader
            let dataset = self_.dataset.borrow(py);
            let client = dataset.client().borrow(py);
            let ctx = client.ctx(py)?;
            let ctx = ctx.bind(py);

            drop(client);
            drop(dataset);

            ctx.call_method1("read_table", (table,))
        }
    }
}

/// Get the resolved entity path filter from content filter expressions.
fn resolved_entity_path_filter(content_filters: &[String]) -> ResolvedEntityPathFilter {
    if content_filters.is_empty() {
        EntityPathFilter::parse_forgiving("/**").resolve_without_substitutions()
    } else {
        let expr = content_filters.join(" ");
        EntityPathFilter::parse_forgiving(&expr).resolve_without_substitutions()
    }
}

/// Build a `ViewContentsSelector` from content filters.
fn build_view_contents(
    schema: &ArrowSchema,
    content_filters: &[String],
) -> Option<ViewContentsSelector> {
    let filter = resolved_entity_path_filter(content_filters);

    let contents: ViewContentsSelector = schema
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
        .filter(|comp| filter.matches(&comp.entity_path))
        .map(|comp| (comp.entity_path.clone(), None))
        .collect();

    if contents.is_empty() {
        None
    } else {
        Some(contents)
    }
}

/// Build a `ViewContentsSelector` from component column selectors.
fn build_view_contents_from_column_selectors(
    schema: &ArrowSchema,
    column_selectors: &[String],
) -> PyResult<Option<ViewContentsSelector>> {
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use re_sorbet::ComponentColumnSelector;

    // Parse all column selectors - fail fast with clear error
    let selectors: Result<Vec<ComponentColumnSelector>, _> = column_selectors
        .iter()
        .map(|s| ComponentColumnSelector::from_str(s))
        .collect();

    let selectors = selectors.map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Invalid column selector format: {e}. Expected format: 'entity_path:component' (e.g., '/world/points:Position3D')"
        ))
    })?;

    // Extract component descriptors from schema and filter by selectors
    let matching_components: Vec<_> = schema
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
        .filter(|comp| selectors.iter().any(|selector| comp.matches(selector)))
        .collect();

    // Build ViewContentsSelector: map entity paths to sets of components
    let mut contents = BTreeMap::new();
    for comp in matching_components {
        contents
            .entry(comp.entity_path.clone())
            .or_insert_with(BTreeSet::new)
            .insert(comp.component);
    }

    // Convert to ViewContentsSelector format
    let view_contents: BTreeMap<_, _> = contents
        .into_iter()
        .map(|(path, components)| (path, Some(components)))
        .collect();

    Ok(if view_contents.is_empty() {
        None
    } else {
        Some(ViewContentsSelector(view_contents))
    })
}

/// Build a table provider for dataframe queries with the given parameters.
#[expect(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn build_dataframe_query_table_provider(
    py: Python<'_>,
    dataset: &Py<PyDatasetEntryInternal>,
    segment_filter: Option<HashSet<String>>,
    content_filters: &[String],
    column_selectors: &Option<Vec<String>>,
    index: Option<String>,
    include_semantically_empty_columns: bool,
    include_tombstone_columns: bool,
    fill_latest_at: bool,
    using_index_values: Option<BTreeSet<TimeInt>>,
) -> PyResult<Arc<dyn TableProvider + Send>> {
    let dataset_ref = dataset.borrow(py);
    let dataset_id = dataset_ref.entry_id();
    let schema = PyDatasetEntryInternal::fetch_arrow_schema(&dataset_ref)?;
    let connection = dataset_ref.client().borrow(py).connection().clone();
    drop(dataset_ref);

    // Build view contents from either column selectors or content filters
    let view_contents = if let Some(selectors) = column_selectors {
        build_view_contents_from_column_selectors(&schema, selectors)?
    } else {
        build_view_contents(&schema, content_filters)
    };
    let segment_ids: Vec<String> = match &segment_filter {
        Some(filter) => filter.iter().cloned().collect(),
        None => vec![],
    };

    let static_only = index.is_none();

    let query_expression = QueryExpression {
        view_contents,
        include_semantically_empty_columns,
        include_tombstone_columns,
        include_static_columns: if static_only {
            re_chunk_store::StaticColumnSelection::StaticOnly
        } else {
            re_chunk_store::StaticColumnSelection::Both
        },
        filtered_index: index.map(Into::into),
        filtered_index_range: None,
        filtered_index_values: None,
        using_index_values,
        filtered_is_not_null: None,
        sparse_fill_strategy: if fill_latest_at {
            SparseFillStrategy::LatestAtGlobal
        } else {
            SparseFillStrategy::None
        },
        selection: None,
    };

    // Capture trace context to propagate into async query execution
    #[cfg(all(feature = "perf_telemetry", not(target_arch = "wasm32")))]
    let trace_headers_opt = {
        let trace_headers = extract_trace_context_from_contextvar(py);
        if trace_headers.traceparent.is_empty() {
            None
        } else {
            Some(trace_headers)
        }
    };
    #[cfg(not(all(feature = "perf_telemetry", not(target_arch = "wasm32"))))]
    let trace_headers_opt = None;

    wait_for_future(py, async move {
        DataframeQueryTableProvider::new(
            connection.origin().clone(),
            connection.connection_registry().clone(),
            dataset_id,
            &query_expression,
            &segment_ids,
            #[cfg(not(target_arch = "wasm32"))]
            trace_headers_opt,
        )
        .await
    })
    .map(|p| Arc::new(p) as Arc<dyn TableProvider + Send>)
    .map_err(to_py_err)
}
