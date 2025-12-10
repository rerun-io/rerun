use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use arrow::datatypes::Schema as ArrowSchema;
use arrow::pyarrow::PyArrowType;
use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::prelude::PyAnyMethods as _;
use pyo3::types::{PyCapsule, PyDict, PyDictMethods as _};
use pyo3::{Bound, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods};
use re_chunk_store::{QueryExpression, SparseFillStrategy, ViewContentsSelector};
use re_datafusion::DataframeQueryTableProvider;
use re_log_types::{EntityPathFilter, ResolvedEntityPathFilter};
use re_sorbet::{ColumnDescriptor, SorbetColumnDescriptors};
use tracing::instrument;

use super::{PyDatasetEntryInternal, PySchemaInternal, to_py_err};
use crate::dataframe::IndexValuesLike;
use crate::utils::{get_tokio_runtime, wait_for_future};

/// A view over a dataset with optional segment and content filters applied lazily.
//TODO(RR-3157): add the ability to filter on components, not just entity paths
#[pyclass(name = "DatasetViewInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PyDatasetViewInternal {
    dataset: Py<PyDatasetEntryInternal>,

    /// Segment filter: `None` means no filtering, `Some` means filter to these segment IDs
    // TODO(RR-3158): the API allows using a datafusion dataframe to specify segments. If actual
    // workflow/performance indicates that it would be useful, we should consider keeping the
    // logical plan and lazily execute it instead of materializing the segment IDs.
    segment_filter: Option<HashSet<String>>,

    /// Content filters: entity path expressions like "/points/**", "-/text/**". If empty,
    /// everything is included.
    content_filters: Vec<String>,
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

    /// Filter schema columns based on content filters.
    fn filter_schema(&self, schema: SorbetColumnDescriptors) -> SorbetColumnDescriptors {
        if self.content_filters.is_empty() {
            return schema;
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

        SorbetColumnDescriptors {
            columns: filtered_columns,
        }
    }

    /// Build a `ViewContentsSelector` from content filters.
    fn build_view_contents(&self, schema: &ArrowSchema) -> Option<ViewContentsSelector> {
        let descriptors = schema
            .fields()
            .iter()
            .map(|field| ColumnDescriptor::try_from_arrow_field(None, field.as_ref()))
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let component_descriptors = descriptors
            .iter()
            .filter_map(|descriptor| {
                if let ColumnDescriptor::Component(component) = descriptor {
                    Some(component)
                } else {
                    None
                }
            })
            .cloned()
            .collect::<Vec<_>>();

        let filter = self.resolved_entity_path_filter();

        // Build contents map: entity_path -> None (all components for that entity)
        let contents: ViewContentsSelector = component_descriptors
            .iter()
            .filter(|comp| filter.matches(&comp.entity_path))
            .map(|comp| (comp.entity_path.clone(), None))
            .collect();

        if contents.is_empty() {
            None
        } else {
            Some(contents)
        }
    }

    /// Convert a TableProvider to a DataFusion DataFrame.
    fn provider_to_dataframe<'py>(
        py: Python<'py>,
        provider: Arc<dyn TableProvider>,
        ctx: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Create a capsule for the table provider
        let runtime = get_tokio_runtime().handle().clone();
        let ffi_provider = FFI_TableProvider::new(provider, true, Some(runtime));
        let capsule_name = cr"datafusion_table_provider".into();
        let capsule = PyCapsule::new(py, ffi_provider, Some(capsule_name))?;

        // Use exec to create a wrapper class with __datafusion_table_provider__
        let builtins = py.import("builtins")?;
        let exec_fn = builtins.getattr("exec")?;
        let globals = pyo3::types::PyDict::new(py);
        globals.set_item("capsule", capsule)?;

        exec_fn.call1((
            r#"
class TableProviderWrapper:
    def __datafusion_table_provider__(self):
        return capsule
wrapper = TableProviderWrapper()
"#,
            globals.clone(),
        ))?;

        let wrapper = globals.get_item("wrapper")?;

        ctx.call_method1("read_table", (wrapper,))
    }

    /// Handle reader with per-segment using_index_values.
    ///
    /// This creates a separate query for each segment with its specific index values,
    /// then unions all the results together.
    #[expect(clippy::too_many_arguments)]
    fn reader_with_using_index_values<'py>(
        py: Python<'py>,
        dataset: &Py<PyDatasetEntryInternal>,
        segment_filter: &Option<HashSet<String>>,
        index: Option<String>,
        include_semantically_empty_columns: bool,
        include_tombstone_columns: bool,
        fill_latest_at: bool,
        view_contents: Option<ViewContentsSelector>,
        using_index_values_dict: Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Parse the dictionary: segment_id -> IndexValuesLike
        let mut per_segment_index_values: HashMap<String, _> = HashMap::new();
        for (key, value) in using_index_values_dict.iter() {
            let segment_id: String = key.extract()?;
            let index_values: IndexValuesLike<'_> = value.extract()?;
            let converted = index_values.to_index_values()?;
            per_segment_index_values.insert(segment_id, converted);
        }

        // Get all valid segment IDs for this view
        let dataset_borrowed = dataset.borrow(py);
        let all_dataset_segment_ids = PyDatasetEntryInternal::segment_ids(dataset_borrowed)?;
        let all_segment_ids: HashSet<String> = match segment_filter {
            Some(filter) => all_dataset_segment_ids
                .into_iter()
                .filter(|id| filter.contains(id))
                .collect(),
            None => all_dataset_segment_ids.into_iter().collect(),
        };

        // Filter to only segments that exist in our view
        let segments_to_query: Vec<_> = per_segment_index_values
            .keys()
            .filter(|seg| all_segment_ids.contains(*seg))
            .cloned()
            .collect();

        // Get dataset info
        let dataset_borrowed = dataset.borrow(py);
        let dataset_id = dataset_borrowed.entry_id();
        let connection = dataset_borrowed.client().borrow(py).connection().clone();
        let client = dataset_borrowed.client().borrow(py);
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py).clone();
        drop(client);
        drop(dataset_borrowed);

        if segments_to_query.is_empty() {
            // Return an empty DataFrame with the correct schema
            // We'll create a provider with empty index values for any segment
            let query_expression = QueryExpression {
                view_contents,
                include_semantically_empty_columns,
                include_tombstone_columns,
                include_static_columns: re_chunk_store::StaticColumnSelection::Both,
                filtered_index: index.map(Into::into),
                filtered_index_range: None,
                filtered_index_values: None,
                using_index_values: Some(std::collections::BTreeSet::new()),
                filtered_is_not_null: None,
                sparse_fill_strategy: if fill_latest_at {
                    SparseFillStrategy::LatestAtGlobal
                } else {
                    SparseFillStrategy::None
                },
                selection: None,
            };

            // Use any single segment from the view (or empty if none)
            let segment_ids: Vec<String> = all_segment_ids.into_iter().take(1).collect();

            let provider = wait_for_future(py, async move {
                DataframeQueryTableProvider::new(
                    connection.origin().clone(),
                    connection.connection_registry().clone(),
                    dataset_id,
                    &query_expression,
                    &segment_ids,
                    #[cfg(not(target_arch = "wasm32"))]
                    None,
                )
                .await
            })
            .map(|p| Arc::new(p) as Arc<dyn TableProvider>)
            .map_err(to_py_err)?;

            return Self::provider_to_dataframe(py, provider, ctx);
        }

        // Create a query for each segment and union the results
        let mut result_df: Option<Bound<'_, PyAny>> = None;

        for segment_id in segments_to_query {
            let index_values = per_segment_index_values
                .remove(&segment_id)
                .expect("segment_id was in segments_to_query");

            let query_expression = QueryExpression {
                view_contents: view_contents.clone(),
                include_semantically_empty_columns,
                include_tombstone_columns,
                include_static_columns: if index.is_none() {
                    re_chunk_store::StaticColumnSelection::StaticOnly
                } else {
                    re_chunk_store::StaticColumnSelection::Both
                },
                filtered_index: index.clone().map(Into::into),
                filtered_index_range: None,
                filtered_index_values: None,
                using_index_values: Some(index_values),
                filtered_is_not_null: None,
                sparse_fill_strategy: if fill_latest_at {
                    SparseFillStrategy::LatestAtGlobal
                } else {
                    SparseFillStrategy::None
                },
                selection: None,
            };

            let segment_ids = vec![segment_id];
            let connection_clone = connection.clone();

            let provider = wait_for_future(py, async move {
                DataframeQueryTableProvider::new(
                    connection_clone.origin().clone(),
                    connection_clone.connection_registry().clone(),
                    dataset_id,
                    &query_expression,
                    &segment_ids,
                    #[cfg(not(target_arch = "wasm32"))]
                    None,
                )
                .await
            })
            .map(|p| Arc::new(p) as Arc<dyn TableProvider>)
            .map_err(to_py_err)?;

            let segment_df = Self::provider_to_dataframe(py, provider, ctx.to_owned())?;

            result_df = Some(match result_df {
                None => segment_df,
                Some(existing) => existing.call_method1("union", (segment_df,))?,
            });
        }

        result_df.ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("No segments to query (unexpected)")
        })
    }
}

#[pymethods]
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
        let base_schema = PyDatasetEntryInternal::fetch_schema(&dataset)?;

        // Apply content filters
        let filtered = self_.filter_schema(base_schema.schema);
        Ok(PySchemaInternal { schema: filtered })
    }

    /// Return the Arrow schema of the data contained in this view.
    #[instrument(skip_all)]
    fn arrow_schema(self_: PyRef<'_, Self>) -> PyResult<PyArrowType<ArrowSchema>> {
        let py = self_.py();
        let dataset = self_.dataset.borrow(py);
        let base_schema = PyDatasetEntryInternal::fetch_arrow_schema(&dataset)?;

        if self_.content_filters.is_empty() {
            return Ok(base_schema.into());
        }

        let filter = self_.resolved_entity_path_filter();

        let filtered_fields: Vec<_> = base_schema
            .fields()
            .iter()
            .filter(|field| {
                let Ok(descriptor) = ColumnDescriptor::try_from_arrow_field(None, field) else {
                    return true;
                };

                //TODO: can we deduplicate this logic?
                match descriptor {
                    ColumnDescriptor::Component(comp) => filter.matches(&comp.entity_path),
                    // Keep row_id and index columns
                    ColumnDescriptor::RowId(_) | ColumnDescriptor::Time(_) => true,
                }
            })
            .cloned()
            .collect();

        Ok(ArrowSchema::new_with_metadata(filtered_fields, base_schema.metadata).into())
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

    /// Create a reader over this DatasetView as a DataFusion DataFrame.
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
    /// using_index_values : dict[str, IndexValuesLike] | None
    ///     If provided, a dictionary mapping segment IDs to the specific index values
    ///     to sample from that segment. Segments not in the dictionary will have no rows.
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
        using_index_values: Option<Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();

        // Get the arrow schema to build view contents
        let dataset = self_.dataset.borrow(py);
        let schema = PyDatasetEntryInternal::fetch_arrow_schema(&dataset)?;
        drop(dataset);

        let view_contents = self_.build_view_contents(&schema);

        let static_only = index.is_none();

        // If using_index_values is provided, we need to handle per-segment queries
        if let Some(using_index_values_dict) = using_index_values {
            return Self::reader_with_using_index_values(
                py,
                &self_.dataset,
                &self_.segment_filter,
                index,
                include_semantically_empty_columns,
                include_tombstone_columns,
                fill_latest_at,
                view_contents,
                using_index_values_dict,
            );
        }

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
            using_index_values: None,
            filtered_is_not_null: None,
            sparse_fill_strategy: if fill_latest_at {
                SparseFillStrategy::LatestAtGlobal
            } else {
                SparseFillStrategy::None
            },
            selection: None,
        };

        // Get segment IDs to use
        let segment_ids: Vec<String> = match &self_.segment_filter {
            Some(filter) => filter.iter().cloned().collect(),
            None => vec![],
        };

        // Create table provider
        let dataset = self_.dataset.borrow(py);
        let dataset_id = dataset.entry_id();
        let connection = dataset.client().borrow(py).connection().clone();
        drop(dataset);

        let provider = wait_for_future(py, async move {
            DataframeQueryTableProvider::new(
                connection.origin().clone(),
                connection.connection_registry().clone(),
                dataset_id,
                &query_expression,
                &segment_ids,
                #[cfg(not(target_arch = "wasm32"))]
                None,
            )
            .await
        })
        .map(|p| Arc::new(p) as Arc<dyn TableProvider>)
        .map_err(to_py_err)?;

        // Register with DataFusion context and return DataFrame
        let dataset = self_.dataset.borrow(py);
        let client = dataset.client().borrow(py);
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py).clone();
        drop(client);
        drop(dataset);

        Self::provider_to_dataframe(py, provider, ctx)
    }

    /// Returns a new DatasetView filtered to the given segment IDs.
    ///
    /// Parameters
    /// ----------
    /// segment_ids : list[str] | DataFrame
    ///     Either a list of segment ID strings or a DataFusion DataFrame
    ///     with a column named 'rerun_segment_id'.
    fn filter_segments(
        self_: PyRef<'_, Self>,
        segment_ids: Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let py = self_.py();

        // Extract segment IDs from input
        let new_filter: HashSet<String> = if segment_ids.is_instance_of::<pyo3::types::PyList>() {
            // Extract as Vec first, then convert to HashSet
            let vec: Vec<String> = segment_ids.extract()?;
            vec.into_iter().collect()
        } else {
            // Assume it's a DataFrame - extract segment IDs from it
            let df = segment_ids;

            // Select the rerun_segment_id column and collect
            let selected = df.call_method1("select", ("rerun_segment_id",))?;
            let collected = selected.call_method0("collect")?;

            let mut ids = HashSet::new();
            // Convert to pyarrow and extract
            let batches: Vec<Bound<'_, PyAny>> = collected.extract()?;
            for batch in batches {
                let column = batch.call_method1("column", ("rerun_segment_id",))?;
                let pylist = column.call_method0("to_pylist")?;
                let items: Vec<String> = pylist.extract()?;
                ids.extend(items);
            }
            ids
        };

        // Intersect with existing filter if present
        let combined_filter = match &self_.segment_filter {
            Some(existing) => existing.intersection(&new_filter).cloned().collect(),
            None => new_filter,
        };

        Py::new(
            py,
            Self::new(
                self_.dataset.clone_ref(py),
                Some(combined_filter),
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
}
