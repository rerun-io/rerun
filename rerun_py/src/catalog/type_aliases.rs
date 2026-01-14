use std::collections::BTreeSet;
use std::str::FromStr as _;

use arrow::array::{ArrayData, Int64Array, make_array};
use arrow::pyarrow::PyArrowType;
use numpy::PyArrayMethods as _;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::PyAnyMethods as _;
use pyo3::{Bound, FromPyObject, PyAny, PyResult, pyclass, pymethods};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sorbet::ComponentColumnSelector;

use crate::catalog::{PyComponentColumnDescriptor, PyComponentColumnSelector};

/// A type alias for any component-column-like object.
//TODO(#9853): rename to `ComponentColumnLike`
#[derive(FromPyObject)]
pub enum AnyComponentColumn {
    #[pyo3(transparent, annotation = "name")]
    Name(String),
    #[pyo3(transparent, annotation = "component_descriptor")]
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyComponentColumn {
    pub fn into_selector(self) -> PyResult<ComponentColumnSelector> {
        match self {
            Self::Name(name) => {
                let sel = ComponentColumnSelector::from_str(&name).map_err(|err| {
                    PyValueError::new_err(format!("Invalid component type '{name}': {err}"))
                })?;

                Ok(sel)
            }
            Self::ComponentDescriptor(desc) => Ok(desc.0.into()),
            Self::ComponentSelector(selector) => Ok(selector.0),
        }
    }
}

/// A type alias for index values.
///
/// This can be any numpy-compatible array of integers, datetime64, or a [`pa.Int64Array`][]
pub enum IndexValuesLike<'py> {
    PyArrow(PyArrowType<ArrayData>),
    NumPy(numpy::PyArrayLike1<'py, i64>),

    // Catch all to support ChunkedArray and other types
    CatchAll(Bound<'py, PyAny>),
}

impl<'py> FromPyObject<'py> for IndexValuesLike<'py> {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        // Try PyArrow first
        if let Ok(pyarrow) = obj.extract::<PyArrowType<ArrayData>>() {
            return Ok(Self::PyArrow(pyarrow));
        }

        // Try numpy i64 array
        if let Ok(numpy) = obj.extract::<numpy::PyArrayLike1<'py, i64>>() {
            return Ok(Self::NumPy(numpy));
        }

        // Check if this is a numpy array with datetime64 dtype
        // First check if it has a dtype attribute to see if it's a numpy array
        if let Ok(dtype) = obj.getattr("dtype")
            && let Ok(dtype_str) = dtype.str()
            && let Ok(dtype_string) = dtype_str.extract::<String>()
        {
            // Check if it's a datetime64 array
            if dtype_string.starts_with("datetime64") {
                // Convert datetime64 to nanoseconds, then view as int64
                let converted_array = obj
                    .call_method1("astype", ("datetime64[ns]",))?
                    .call_method0("view")?
                    .call_method1("astype", ("int64",))?;

                if let Ok(i64_array) = converted_array.extract::<numpy::PyArrayLike1<'py, i64>>() {
                    return Ok(Self::NumPy(i64_array));
                }
            }
        }

        // Fall back to catch all
        Ok(Self::CatchAll(obj.clone()))
    }
}

impl IndexValuesLike<'_> {
    pub fn to_index_values(&self) -> PyResult<BTreeSet<re_chunk_store::TimeInt>> {
        match self {
            Self::PyArrow(array) => {
                let array = make_array(array.0.clone());

                let int_array = array.downcast_array_ref::<Int64Array>().ok_or_else(|| {
                    PyTypeError::new_err("pyarrow.Array for IndexValuesLike must be of type int64.")
                })?;

                let values: BTreeSet<re_chunk_store::TimeInt> = int_array
                    .iter()
                    .map(|v| {
                        v.map_or_else(
                            || re_chunk_store::TimeInt::STATIC,
                            // The use of temporal here should be fine even if the data is
                            // not actually temporal. The important thing is we are converting
                            // from an i64 input
                            re_chunk_store::TimeInt::new_temporal,
                        )
                    })
                    .collect();

                if values.len() != int_array.len() {
                    return Err(PyValueError::new_err("Index values must be unique."));
                }

                Ok(values)
            }
            Self::NumPy(array) => {
                let values: BTreeSet<re_chunk_store::TimeInt> = array
                    .readonly()
                    .as_array()
                    .iter()
                    // The use of temporal here should be fine even if the data is
                    // not actually temporal. The important thing is we are converting
                    // from an i64 input
                    .map(|v| re_chunk_store::TimeInt::new_temporal(*v))
                    .collect();

                if values.len() != array.len()? {
                    return Err(PyValueError::new_err("Index values must be unique."));
                }

                Ok(values)
            }
            Self::CatchAll(any) => {
                // If any has the `.chunks` attribute, we can try to try each chunk as pyarrow array
                match any.getattr("chunks") {
                    Ok(chunks) => {
                        let mut values = BTreeSet::new();
                        for chunk in chunks.try_iter()? {
                            let chunk = chunk?.extract::<PyArrowType<ArrayData>>()?;
                            let array = make_array(chunk.0.clone());

                            let int_array =
                                array.downcast_array_ref::<Int64Array>().ok_or_else(|| {
                                    PyTypeError::new_err(
                                        "pyarrow.Array for IndexValuesLike must be of type int64.",
                                    )
                                })?;

                            values.extend(
                                int_array
                                    .iter()
                                    .map(|v| {
                                        v.map_or_else(
                                            || re_chunk_store::TimeInt::STATIC,
                                            // The use of temporal here should be fine even if the data is
                                            // not actually temporal. The important thing is we are converting
                                            // from an i64 input
                                            re_chunk_store::TimeInt::new_temporal,
                                        )
                                    })
                                    .collect::<BTreeSet<_>>(),
                            );
                        }
                        if values.len() != any.len()? {
                            return Err(PyValueError::new_err("Index values must be unique."));
                        }
                        Ok(values)
                    }
                    Err(err) => Err(PyTypeError::new_err(format!(
                        "IndexValuesLike must be a pyarrow.Array, pyarrow.ChunkedArray, numpy.ndarray of int64, or numpy.ndarray of datetime64. {err}"
                    ))),
                }
            }
        }
    }
}

/// A Python wrapper for testing [`IndexValuesLike`] extraction functionality.
///
/// This wrapper allows testing the `extract_bound` functionality by providing
/// a Python-accessible interface to create and convert index values.
#[pyclass(
    frozen,
    name = "_IndexValuesLikeInternal",
    module = "rerun_bindings.rerun_bindings",
    hash,
    eq
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyIndexValuesLikeInternal {
    // Store the converted values instead of the lifetime-bound enum
    values: BTreeSet<re_chunk_store::TimeInt>,
}

#[pymethods]
impl PyIndexValuesLikeInternal {
    /// Create a new `IndexValuesLike` from a Python object.
    ///
    /// Parameters
    /// ----------
    /// obj : IndexValuesLike
    ///     A PyArrow Array, NumPy array of int64/datetime64, or ChunkedArray.
    #[new]
    #[pyo3(text_signature = "(self, values)")]
    fn new(values: Bound<'_, PyAny>) -> PyResult<Self> {
        let index_values_like = IndexValuesLike::extract_bound(&values)?;
        let values = index_values_like.to_index_values()?;
        Ok(Self { values })
    }

    /// Get the extracted index values.
    ///
    /// Returns
    /// -------
    /// npt.NDArray[np.int64]
    ///     The extracted index values as a list of integers.
    fn to_index_values(&self) -> Vec<i64> {
        self.values
            .iter()
            .map(|time_int| time_int.as_i64())
            .collect()
    }

    /// Get the number of unique values.
    ///
    /// Returns
    /// -------
    /// int
    ///     The number of unique index values.
    fn len(&self) -> usize {
        self.values.len()
    }

    fn __repr__(&self) -> String {
        format!("IndexValuesLike({} values)", self.values.len())
    }
}
