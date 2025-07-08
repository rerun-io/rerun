use std::collections::BTreeSet;
use std::str::FromStr as _;

use arrow::array::{ArrayData, Int64Array, make_array};
use arrow::pyarrow::PyArrowType;
use numpy::PyArrayMethods as _;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::PyAnyMethods as _;
use pyo3::{Bound, FromPyObject, PyAny, PyResult};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sorbet::{ColumnDescriptor, ColumnSelector, ComponentColumnSelector, TimeColumnSelector};

use super::{
    PyComponentColumnDescriptor, PyComponentColumnSelector, PyIndexColumnDescriptor,
    PyIndexColumnSelector,
};

/// A type alias for any component-column-like object.
#[derive(FromPyObject)]
pub enum AnyColumn {
    #[pyo3(transparent, annotation = "name")]
    Name(String),
    #[pyo3(transparent, annotation = "index_descriptor")]
    IndexDescriptor(PyIndexColumnDescriptor),
    #[pyo3(transparent, annotation = "index_selector")]
    IndexSelector(PyIndexColumnSelector),
    #[pyo3(transparent, annotation = "component_descriptor")]
    ComponentDescriptor(PyComponentColumnDescriptor),
    #[pyo3(transparent, annotation = "component_selector")]
    ComponentSelector(PyComponentColumnSelector),
}

impl AnyColumn {
    pub fn into_selector(self) -> PyResult<ColumnSelector> {
        match self {
            Self::Name(name) => {
                if !name.contains(':') && !name.contains('/') {
                    Ok(ColumnSelector::Time(TimeColumnSelector::from(name)))
                } else {
                    let sel = ComponentColumnSelector::from_str(&name).map_err(|err| {
                        PyValueError::new_err(format!("Invalid component type '{name}': {err}"))
                    })?;

                    Ok(ColumnSelector::Component(sel))
                }
            }
            Self::IndexDescriptor(desc) => Ok(ColumnDescriptor::Time(desc.0).into()),
            Self::IndexSelector(selector) => Ok(selector.0.into()),
            Self::ComponentDescriptor(desc) => Ok(ColumnDescriptor::Component(desc.0).into()),
            Self::ComponentSelector(selector) => Ok(ColumnSelector::Component(selector.0)),
        }
    }
}

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
    #[allow(dead_code)]
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
/// This can be any numpy-compatible array of integers, or a [`pa.Int64Array`][]
#[derive(FromPyObject)]
pub enum IndexValuesLike<'py> {
    PyArrow(PyArrowType<ArrayData>),
    NumPy(numpy::PyArrayLike1<'py, i64>),

    // Catch all to support ChunkedArray and other types
    #[pyo3(transparent)]
    CatchAll(Bound<'py, PyAny>),
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
                        "IndexValuesLike must be a pyarrow.Array, pyarrow.ChunkedArray, or numpy.ndarray. {err}"
                    ))),
                }
            }
        }
    }
}
