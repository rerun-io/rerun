use std::collections::BTreeSet;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::PyAnyMethods as _;
use pyo3::types::PyDict;
use pyo3::{Bound, Py, PyAny, PyResult, pyclass, pymethods};
use re_chunk::ComponentIdentifier;
use re_chunk_store::{
    ChunkStoreHandle, QueryExpression, SparseFillStrategy, StaticColumnSelection,
    ViewContentsSelector,
};
use re_dataframe::{QueryEngine, StorageEngine};
use re_log_types::EntityPathFilter;
use re_sorbet::TimeColumnSelector;

use super::{PyRecordingView, PySchema};

/// A single Rerun recording.
///
/// This can be loaded from an RRD file using [`load_recording()`][rerun.dataframe.load_recording].
///
/// A recording is a collection of data that was logged to Rerun. This data is organized
/// as a column for each index (timeline) and each entity/component pair that was logged.
///
/// You can examine the [`.schema()`][rerun.dataframe.Recording.schema] of the recording to see
/// what data is available, or create a [`RecordingView`][rerun.dataframe.RecordingView] to
/// to retrieve the data.
#[pyclass(name = "Recording")]
pub struct PyRecording {
    pub(crate) store: ChunkStoreHandle,
    pub(crate) cache: re_dataframe::QueryCacheHandle,
}

#[derive(Clone)]
pub enum PyRecordingHandle {
    Local(std::sync::Arc<Py<PyRecording>>),
    // TODO(rerun-io/dataplatform#405): interface with remote data needs to be reimplemented
    //Remote(std::sync::Arc<Py<PyRemoteRecording>>),
}

impl PyRecording {
    pub fn engine(&self) -> QueryEngine<StorageEngine> {
        // Safety: this is all happening in the context of a python client using the dataframe API,
        // there is no reason to worry about handle leakage whatsoever.
        #[allow(unsafe_code)]
        let engine = unsafe { StorageEngine::new(self.store.clone(), self.cache.clone()) };

        QueryEngine { engine }
    }

    /// Convert a `ViewContentsLike` into a `ViewContentsSelector`.
    ///
    /// ```python
    /// ViewContentsLike = Union[str, Dict[str, Union[str, Sequence[str]]]]
    /// ```
    ///
    /// We cant do this with the normal `FromPyObject` mechanisms because we want access to the
    /// `QueryEngine` to resolve the entity paths.
    fn extract_contents_expr(
        &self,
        expr: Bound<'_, PyAny>,
    ) -> PyResult<re_chunk_store::ViewContentsSelector> {
        let engine = self.engine();

        if let Ok(expr) = expr.extract::<String>() {
            // `str`

            let path_filter =
                EntityPathFilter::parse_strict(&expr)
                    .map_err(|err| {
                        PyValueError::new_err(format!(
                            "Could not interpret `contents` as a ViewContentsLike. Failed to parse {expr}: {err}.",
                        ))
                    })?;

            let contents = engine
                .iter_entity_paths_sorted(&path_filter)
                .map(|p| (p, None))
                .collect();

            Ok(contents)
        } else if let Ok(dict) = expr.downcast::<PyDict>() {
            // `Union[str, Sequence[str]]]`

            let mut contents = ViewContentsSelector::default();

            for (key, value) in dict {
                let key = key.extract::<String>().map_err(|_err| {
                    PyTypeError::new_err(
                        format!("Could not interpret `contents` as a ViewContentsLike. Key: {key} is not a path expression."),
                    )
                })?;

                let path_filter = EntityPathFilter::parse_strict(&key).map_err(|err| {
                    PyValueError::new_err(format!(
                        "Could not interpret `contents` as a ViewContentsLike. Failed to parse {key}: {err}.",
                    ))
                })?;

                let component_strs: BTreeSet<ComponentIdentifier> = if let Ok(component) =
                    value.extract::<String>()
                {
                    std::iter::once(component.into()).collect()
                } else if let Ok(components) = value.extract::<Vec<String>>() {
                    components.into_iter().map(Into::into).collect()
                } else {
                    return Err(PyTypeError::new_err(format!(
                        "Could not interpret `contents` as a ViewContentsLike. Value: {value} is not a str or Sequence[str]."
                    )));
                };

                contents.append(
                    &mut engine
                        .iter_entity_paths_sorted(&path_filter)
                        .map(|entity_path| (entity_path, Some(component_strs.clone())))
                        .collect(),
                );
            }

            Ok(contents)
        } else {
            return Err(PyTypeError::new_err(
                "Could not interpret `contents` as a ViewContentsLike. Top-level type must be a string or a dictionary.",
            ));
        }
    }
}

#[pymethods]
impl PyRecording {
    /// The schema describing all the columns available in the recording.
    fn schema(&self) -> PySchema {
        PySchema {
            schema: self.store.read().schema().into(),
        }
    }

    #[allow(rustdoc::private_doc_tests, rustdoc::invalid_rust_codeblocks)]
    /// Create a [`RecordingView`][rerun.dataframe.RecordingView] of the recording according to a particular index and content specification.
    ///
    /// The only type of index currently supported is the name of a timeline, or `None` (see below
    /// for details).
    ///
    /// The view will only contain a single row for each unique value of the index
    /// that is associated with a component column that was included in the view.
    /// Component columns that are not included via the view contents will not
    /// impact the rows that make up the view. If the same entity / component pair
    /// was logged to a given index multiple times, only the most recent row will be
    /// included in the view, as determined by the `row_id` column. This will
    /// generally be the last value logged, as row_ids are guaranteed to be
    /// monotonically increasing when data is sent from a single process.
    ///
    /// If `None` is passed as the index, the view will contain only static columns (among those
    /// specified) and no index columns. It will also contain a single row per partition.
    ///
    /// Parameters
    /// ----------
    /// index : str | None
    ///     The index to use for the view. This is typically a timeline name. Use `None` to query static data only.
    /// contents : ViewContentsLike
    ///     The content specification for the view.
    ///
    ///     This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
    ///     specifying multiple content-expressions and a respective list of components to select within
    ///     that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
    /// include_semantically_empty_columns : bool, optional
    ///     Whether to include columns that are semantically empty, by default `False`.
    ///
    ///     Semantically empty columns are components that are `null` or empty `[]` for every row in the recording.
    /// include_indicator_columns : bool, optional
    ///     Whether to include indicator columns, by default `False`.
    ///
    ///     Indicator columns are components used to represent the presence of an archetype within an entity.
    /// include_tombstone_columns : bool, optional
    ///     Whether to include tombstone columns, by default `False`.
    ///
    ///     Tombstone columns are components used to represent clears. However, even without the clear
    ///     tombstone columns, the view will still apply the clear semantics when resolving row contents.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     The view of the recording.
    ///
    /// Examples
    /// --------
    /// All the data in the recording on the timeline "my_index":
    /// ```python
    /// recording.view(index="my_index", contents="/**")
    /// ```
    ///
    /// Just the Position3D components in the "points" entity:
    /// ```python
    /// recording.view(index="my_index", contents={"points": "Position3D"})
    /// ```
    #[allow(clippy::fn_params_excessive_bools)]
    #[pyo3(signature = (
        *,
        index,
        contents,
        include_semantically_empty_columns = false,
        include_indicator_columns = false,
        include_tombstone_columns = false,
    ))]
    fn view(
        slf: Bound<'_, Self>,
        index: Option<&str>,
        contents: Bound<'_, PyAny>,
        include_semantically_empty_columns: bool,
        include_indicator_columns: bool,
        include_tombstone_columns: bool,
    ) -> PyResult<PyRecordingView> {
        let static_only = index.is_none();

        let borrowed_self = slf.borrow();

        // Look up the type of the timeline
        let filtered_index = index.map(|index| {
            let selector = TimeColumnSelector::from(index);
            let time_column = borrowed_self.store.read().resolve_time_selector(&selector);
            *time_column.timeline().name()
        });

        let contents = borrowed_self.extract_contents_expr(contents)?;

        let query = QueryExpression {
            view_contents: Some(contents),
            include_semantically_empty_columns,
            include_indicator_columns,
            include_tombstone_columns,
            include_static_columns: if static_only {
                StaticColumnSelection::StaticOnly
            } else {
                StaticColumnSelection::Both
            },
            filtered_index,
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values: None,
            filtered_is_not_null: None,
            //TODO(#10327): this should not be necessary!
            sparse_fill_strategy: if static_only {
                SparseFillStrategy::LatestAtGlobal
            } else {
                SparseFillStrategy::None
            },
            selection: None,
        };

        let recording = slf.unbind();

        Ok(PyRecordingView {
            recording: PyRecordingHandle::Local(std::sync::Arc::new(recording)),
            query_expression: query,
        })
    }

    /// The recording ID of the recording.
    fn recording_id(&self) -> String {
        self.store.read().id().as_str().to_owned()
    }

    /// The application ID of the recording.
    fn application_id(&self) -> PyResult<String> {
        Ok(self
            .store
            .read()
            .store_info()
            .ok_or(PyValueError::new_err(
                "Recording is missing application id.",
            ))?
            .application_id
            .as_str()
            .to_owned())
    }
}
