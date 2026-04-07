use arrow::array::{Array as _, ArrayData, ArrayRef, ListArray, make_array};
use arrow::pyarrow::{PyArrowType, ToPyArrow as _};
use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use re_lenses_core::{DynExpr, Selector};

/// Register the selector class.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySelectorInternal>()?;
    Ok(())
}

#[pyclass(name = "SelectorInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PySelectorInternal {
    selector: Selector<DynExpr>,
}

/// Wrap a Python callable into a closure compatible with [`re_lenses_core::IntoDynExpr`].
fn wrap_py_callable(
    callback: Py<PyAny>,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, re_lenses_core::combinators::Error>
+ Send
+ Sync
+ 'static {
    move |source: &ArrayRef| {
        Python::attach(|py| {
            let py_array = source.to_data().to_pyarrow(py).map_err(|err| {
                re_lenses_core::combinators::Error::Other(format!(
                    "Failed to convert array to PyArrow: {err}"
                ))
            })?;

            let result = callback.call1(py, (py_array,)).map_err(|err| {
                re_lenses_core::combinators::Error::Other(format!("Python callback failed: {err}"))
            })?;

            if result.is_none(py) {
                return Ok(None);
            }

            let array_data: PyArrowType<ArrayData> = result.extract(py).map_err(|err| {
                re_lenses_core::combinators::Error::Other(format!(
                    "Failed to convert callback result to Arrow: {err}"
                ))
            })?;
            Ok(Some(make_array(array_data.0)))
        })
    }
}

#[pymethods]
impl PySelectorInternal {
    #[new]
    #[pyo3(text_signature = "(self, query)")]
    fn new(query: &str) -> PyResult<Self> {
        let selector: Selector<DynExpr> = Selector::parse(query)
            .map_err(|err| PyValueError::new_err(format!("Failed to parse selector: {err}")))?
            .into();
        Ok(Self { selector })
    }

    /// Execute this selector against a pyarrow array.
    fn execute(&self, py: Python<'_>, source: PyArrowType<ArrayData>) -> PyResult<Py<PyAny>> {
        let array: ArrayRef = make_array(source.0);
        let result = self
            .selector
            .execute(array)
            .map_err(|err| PyRuntimeError::new_err(format!("Selector execution failed: {err}")))?;
        match result {
            Some(arr) => arr.to_data().to_pyarrow(py).map(|obj| obj.unbind()),
            None => Ok(py.None()),
        }
    }

    /// Execute this selector against each row of a pyarrow list array.
    ///
    /// The output is guaranteed to have the same number of rows as the input.
    fn execute_per_row(
        &self,
        py: Python<'_>,
        source: PyArrowType<ArrayData>,
    ) -> PyResult<Py<PyAny>> {
        let array: ArrayRef = make_array(source.0);
        let list_array = array.as_any().downcast_ref::<ListArray>().ok_or_else(|| {
            PyTypeError::new_err(format!("expected a ListArray, got {:?}", array.data_type()))
        })?;
        let result = self
            .selector
            .execute_per_row(list_array)
            .map_err(|err| PyRuntimeError::new_err(format!("Selector execution failed: {err}")))?;
        match result {
            Some(arr) => arr.to_data().to_pyarrow(py).map(|obj| obj.unbind()),
            None => Ok(py.None()),
        }
    }

    /// Pipe this selector into a transformation function or another selector.
    ///
    /// The function must accept a pyarrow array and return a pyarrow array or None.
    fn pipe(&self, py: Python<'_>, func: Py<PyAny>) -> PyResult<Self> {
        // Check if `func` is another selector.
        if let Ok(other) = func.extract::<PyRef<'_, Self>>(py) {
            return Ok(Self {
                selector: self.selector.clone().pipe(other.selector.clone()),
            });
        }

        // Otherwise, it must be a callable.
        if !func.bind(py).is_callable() {
            return Err(PyTypeError::new_err(
                "pipe() argument must be a callable or a Selector",
            ));
        }

        Ok(Self {
            selector: self.selector.clone().pipe(wrap_py_callable(func)),
        })
    }

    fn __repr__(&self) -> String {
        let s = self.selector.to_string_lossy();
        format!("Selector({s:?})")
    }

    fn __str__(&self) -> String {
        self.selector.to_string_lossy()
    }
}
