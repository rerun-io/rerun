#![expect(deprecated)] // False positive due to macro

use arrow::array::{RecordBatchIterator, RecordBatchReader};
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods as _, PyTupleMethods as _};
use pyo3::types::PyTuple;
use pyo3::{Bound, PyRef, PyResult, Python, pyclass, pymethods};

use re_chunk_store::{QueryExpression, SparseFillStrategy};
use re_log_types::ResolvedTimeRange;
use re_sorbet::{ColumnDescriptor, ColumnSelector};

use super::{
    AnyColumn, AnyComponentColumn, IndexValuesLike, PyRecording, PyRecordingHandle, PySchema,
};
use crate::utils::py_rerun_warn_cstr;

/// A view of a recording restricted to a given index, containing a specific set of entities and components.
///
/// See [`Recording.view(…)`][rerun.dataframe.Recording.view] for details on how to create a `RecordingView`.
///
/// Note: `RecordingView` APIs never mutate the underlying view. Instead, they
/// always return new views with the requested modifications applied.
///
/// The view will only contain a single row for each unique value of the index
/// that is associated with a component column that was included in the view.
/// Component columns that are not included via the view contents will not
/// impact the rows that make up the view. If the same entity / component pair
/// was logged to a given index multiple times, only the most recent row will be
/// included in the view, as determined by the `row_id` column. This will
/// generally be the last value logged, as row_ids are guaranteed to be
/// monotonically increasing when data is sent from a single process.
#[pyclass(name = "RecordingView")]
#[derive(Clone)]
pub struct PyRecordingView {
    pub(crate) recording: PyRecordingHandle,

    pub(crate) query_expression: QueryExpression,
}

impl PyRecordingView {
    fn select_args(
        args: &Bound<'_, PyTuple>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<Option<Vec<ColumnSelector>>> {
        // Coerce the arguments into a list of `ColumnSelector`s
        let args: Vec<AnyColumn> = args
            .iter()
            .map(|arg| arg.extract::<AnyColumn>())
            .collect::<PyResult<_>>()?;

        if columns.is_some() && !args.is_empty() {
            return Err(PyValueError::new_err(
                "Cannot specify both `columns` and `args` in `select`.",
            ));
        }

        let columns = columns.or(if !args.is_empty() { Some(args) } else { None });

        columns
            .map(|cols| {
                cols.into_iter()
                    .map(|col| col.into_selector())
                    .collect::<PyResult<_>>()
            })
            .transpose()
    }
}

/// A view of a recording restricted to a given index, containing a specific set of entities and components.
///
/// Can only be created by calling `view(...)` on a `Recording`.
///
/// The only type of index currently supported is the name of a timeline.
///
/// The view will only contain a single row for each unique value of the index. If the same entity / component pair
/// was logged to a given index multiple times, only the most recent row will be included in the view, as determined
/// by the `row_id` column. This will generally be the last value logged, as row_ids are guaranteed to be monotonically
/// increasing when data is sent from a single process.
#[pymethods]
impl PyRecordingView {
    /// The schema describing all the columns available in the view.
    ///
    /// This schema will only contain the columns that are included in the view via
    /// the view contents.
    fn schema(&self, py: Python<'_>) -> PySchema {
        match &self.recording {
            PyRecordingHandle::Local(recording) => {
                let borrowed: PyRef<'_, PyRecording> = recording.borrow(py);
                let engine = borrowed.engine();

                let mut query_expression = self.query_expression.clone();
                query_expression.selection = None;

                PySchema {
                    schema: engine.schema_for_query(&query_expression).into(),
                }
            }
        }
    }

    /// Select the columns from the view.
    ///
    /// If no columns are provided, all available columns will be included in
    /// the output.
    ///
    /// The selected columns do not change the rows that are included in the
    /// view. The rows are determined by the index values and the components
    /// that were included in the view contents, or can be overridden with
    /// [`.using_index_values()`][rerun.dataframe.RecordingView.using_index_values].
    ///
    /// If a column was not provided with data for a given row, it will be
    /// `null` in the output.
    ///
    /// The output is a [`pyarrow.RecordBatchReader`][] that can be used to read
    /// out the data.
    ///
    /// Parameters
    /// ----------
    /// *args : AnyColumn
    ///     The columns to select.
    /// columns : Optional[Sequence[AnyColumn]], optional
    ///     Alternatively the columns to select can be provided as a sequence.
    ///
    /// Returns
    /// -------
    /// pa.RecordBatchReader
    ///     A reader that can be used to read out the selected data.
    #[pyo3(signature = (
        *args,
        columns = None
    ))]
    fn select(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let mut query_expression = self.query_expression.clone();
        query_expression.selection = Self::select_args(args, columns)?;

        match &self.recording {
            PyRecordingHandle::Local(recording) => {
                let borrowed = recording.borrow(py);
                let engine = borrowed.engine();

                let query_handle = engine.query(query_expression);

                // If the only contents found are static, we might need to warn the user since
                // this means we won't naturally have any rows in the result.
                let available_data_columns = &query_handle.view_contents().components;

                // We only consider all contents static if there at least some columns
                let all_contents_are_static = !available_data_columns.is_empty()
                    && available_data_columns.iter().all(|c| c.is_static());

                // Additionally, we only want to warn if the user actually tried to select some
                // of the static columns. Otherwise the fact that there are no results shouldn't
                // be surprising.
                let selected_data_columns = query_handle
                    .selected_contents()
                    .iter()
                    .map(|(_, col)| col)
                    .filter(|c| matches!(c, ColumnDescriptor::Component(_)))
                    .collect::<Vec<_>>();

                let any_selected_data_is_static =
                    selected_data_columns.iter().any(|c| c.is_static());

                if self.query_expression.using_index_values.is_none()
                    && all_contents_are_static
                    && any_selected_data_is_static
                    && self.query_expression.filtered_index.is_some()
                {
                    py_rerun_warn_cstr(c"RecordingView::select: tried to select static data, but no non-static contents generated an index value on this timeline. No results will be returned. Either include non-static data or consider using `select_static()` instead.")?;
                }

                let schema = query_handle.schema().clone();

                let reader =
                    RecordBatchIterator::new(query_handle.into_batch_iter().map(Ok), schema);
                Ok(PyArrowType(Box::new(reader)))
            }
        }
    }

    /// Select only the static columns from the view.
    ///
    /// Because static data has no associated index values it does not cause a
    /// row to be generated in the output. If your view only contains static data
    /// this method allows you to select it without needing to provide index values.
    ///
    /// This method will always return a single row.
    ///
    /// Any non-static columns that are included in the selection will generate a warning
    /// and produce empty columns.
    ///
    ///
    /// Parameters
    /// ----------
    /// *args : AnyColumn
    ///     The columns to select.
    /// columns : Optional[Sequence[AnyColumn]], optional
    ///     Alternatively the columns to select can be provided as a sequence.
    ///
    /// Returns
    /// -------
    /// pa.RecordBatchReader
    ///     A reader that can be used to read out the selected data.
    //TODO(#10335): remove deprecated method
    #[pyo3(signature = (
        *args,
        columns = None
    ))]
    fn select_static(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        columns: Option<Vec<AnyColumn>>,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let mut query_expression = self.query_expression.clone();
        // This is a static selection, so we clear the filtered index
        query_expression.filtered_index = None;

        //TODO(#10327): this should not be necessary!
        query_expression.sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;

        // If no columns provided, select all static columns
        let static_columns = Self::select_args(args, columns)
            .transpose()
            .unwrap_or_else(|| {
                Ok(self
                    .schema(py)
                    .schema
                    .component_columns()
                    .filter(|col| col.is_static())
                    .map(|col| ColumnDescriptor::Component(col.clone()).into())
                    .collect())
            })?;

        query_expression.selection = Some(static_columns);

        match &self.recording {
            PyRecordingHandle::Local(recording) => {
                let borrowed = recording.borrow(py);
                let engine = borrowed.engine();

                let query_handle = engine.query(query_expression);

                let non_static_cols = query_handle
                    .selected_contents()
                    .iter()
                    .filter(|(_, col)| !col.is_static())
                    .collect::<Vec<_>>();

                if !non_static_cols.is_empty() {
                    return Err(PyValueError::new_err(format!(
                        "Static selection resulted in non-static columns: {non_static_cols:?}",
                    )));
                }

                let schema = query_handle.schema().clone();

                let reader =
                    RecordBatchIterator::new(query_handle.into_batch_iter().map(Ok), schema);

                Ok(PyArrowType(Box::new(reader)))
            }
        }
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data between the given index sequence numbers.
    ///
    /// This range is inclusive and will contain both the value at the start and the value at the end.
    ///
    /// The view must be of a sequential index type to use this method.
    ///
    /// Parameters
    /// ----------
    /// start : int
    ///     The inclusive start of the range.
    /// end : int
    ///     The inclusive end of the range.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data within the specified range.
    ///
    ///     The original view will not be modified.
    fn filter_range_sequence(&self, start: i64, end: i64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            // TODO(#9084): do we need this check? If so, how can we accomplish it?
            // Some(filtered_index) if filtered_index.typ() != TimeType::Sequence => {
            //     return Err(PyValueError::new_err(format!(
            //         "Index for {} is not a sequence.",
            //         filtered_index.name()
            //     )));
            // }
            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = if let Ok(seq) = re_chunk::TimeInt::try_from(start) {
            seq
        } else {
            re_log::error!(
                illegal_value = start,
                new_value = re_chunk::TimeInt::MIN.as_i64(),
                "set_time_sequence() called with illegal value - clamped to minimum legal value"
            );
            re_chunk::TimeInt::MIN
        };

        let end = if let Ok(seq) = re_chunk::TimeInt::try_from(end) {
            seq
        } else {
            re_log::error!(
                illegal_value = end,
                new_value = re_chunk::TimeInt::MAX.as_i64(),
                "set_time_sequence() called with illegal value - clamped to maximum legal value"
            );
            re_chunk::TimeInt::MAX
        };

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data between the given index values expressed as seconds.
    ///
    /// This range is inclusive and will contain both the value at the start and the value at the end.
    ///
    /// The view must be of a temporal index type to use this method.
    ///
    /// Parameters
    /// ----------
    /// start : int
    ///     The inclusive start of the range.
    /// end : int
    ///     The inclusive end of the range.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data within the specified range.
    ///
    ///     The original view will not be modified.
    fn filter_range_secs(&self, start: f64, end: f64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            // TODO(#9084): do we need this check? If so, how can we accomplish it?
            // Some(filtered_index) if filtered_index.typ() != TimeType::Time => {
            //     return Err(PyValueError::new_err(format!(
            //         "Index for {} is not temporal.",
            //         filtered_index.name()
            //     )));
            // }
            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = re_log_types::Timestamp::from_secs_since_epoch(start);
        let end = re_log_types::Timestamp::from_secs_since_epoch(end);

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    /// DEPRECATED: Renamed to `filter_range_secs`.
    #[deprecated(since = "0.23.0", note = "Renamed to `filter_range_secs`")]
    fn filter_range_seconds(&self, start: f64, end: f64) -> PyResult<Self> {
        self.filter_range_secs(start, end)
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data between the given index values expressed as nanoseconds.
    ///
    /// This range is inclusive and will contain both the value at the start and the value at the end.
    ///
    /// The view must be of a temporal index type to use this method.
    ///
    /// Parameters
    /// ----------
    /// start : int
    ///     The inclusive start of the range.
    /// end : int
    ///     The inclusive end of the range.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data within the specified range.
    ///
    ///     The original view will not be modified.
    fn filter_range_nanos(&self, start: i64, end: i64) -> PyResult<Self> {
        match self.query_expression.filtered_index.as_ref() {
            // TODO(#9084): do we need this?
            // Some(filtered_index) if filtered_index.typ() != TimeType::Time => {
            //     return Err(PyValueError::new_err(format!(
            //         "Index for {} is not temporal.",
            //         filtered_index.name()
            //     )));
            // }
            Some(_) => {}

            None => {
                return Err(PyValueError::new_err(
                    "Specify an index to filter on first.".to_owned(),
                ));
            }
        }

        let start = re_log_types::Timestamp::from_nanos_since_epoch(start);
        let end = re_log_types::Timestamp::from_nanos_since_epoch(end);

        let resolved = ResolvedTimeRange::new(start, end);

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_range = Some(resolved);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include data at the provided index values.
    ///
    /// The index values returned will be the intersection between the provided values and the
    /// original index values.
    ///
    /// This requires index values to be a precise match. Index values in Rerun are
    /// represented as i64 sequence counts or nanoseconds. This API does not expose an interface
    /// in floating point seconds, as the numerical conversion would risk false mismatches.
    ///
    /// Parameters
    /// ----------
    /// values : IndexValuesLike
    ///     The index values to filter by.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data at the specified index values.
    ///
    ///     The original view will not be modified.
    fn filter_index_values(&self, values: IndexValuesLike<'_>) -> PyResult<Self> {
        let values = values.to_index_values()?;

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_index_values = Some(values);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Filter the view to only include rows where the given component column is not null.
    ///
    /// This corresponds to rows for index values where this component was provided to Rerun explicitly
    /// via `.log()` or `.send_columns()`.
    ///
    /// Parameters
    /// ----------
    /// column : AnyComponentColumn
    ///     The component column to filter by.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing only the data where the specified component column is not null.
    ///
    ///     The original view will not be modified.
    fn filter_is_not_null(&self, column: AnyComponentColumn) -> PyResult<Self> {
        let column = column.into_selector();

        let mut query_expression = self.query_expression.clone();
        query_expression.filtered_is_not_null = Some(column?);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Replace the index in the view with the provided values.
    ///
    /// The output view will always have the same number of rows as the provided values, even if
    /// those rows are empty. Use with [`.fill_latest_at()`][rerun.dataframe.RecordingView.fill_latest_at]
    /// to populate these rows with the most recent data.
    ///
    /// This requires index values to be a precise match. Index values in Rerun are
    /// represented as i64 sequence counts or nanoseconds. This API does not expose an interface
    /// in floating point seconds, as the numerical conversion would risk false mismatches.
    ///
    /// Parameters
    /// ----------
    /// values : IndexValuesLike
    ///     The index values to use.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view containing the provided index values.
    ///
    ///     The original view will not be modified.
    fn using_index_values(&self, values: IndexValuesLike<'_>) -> PyResult<Self> {
        let values = values.to_index_values()?;

        let mut query_expression = self.query_expression.clone();
        query_expression.using_index_values = Some(values);

        Ok(Self {
            recording: self.recording.clone(),
            query_expression,
        })
    }

    #[allow(rustdoc::private_doc_tests)]
    /// Populate any null values in a row with the latest valid data according to the index.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     A new view with the null values filled in.
    ///
    ///     The original view will not be modified.
    fn fill_latest_at(&self) -> Self {
        let mut query_expression = self.query_expression.clone();
        query_expression.sparse_fill_strategy = SparseFillStrategy::LatestAtGlobal;

        Self {
            recording: self.recording.clone(),
            query_expression,
        }
    }
}
