use arrow::datatypes::DataType;
use datafusion::logical_expr::Volatility;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::PyModule;
use pyo3::types::{PyAnyMethods as _, PyCFunction, PyDict, PyList, PyString, PyTuple};
use pyo3::{Bound, IntoPyObject as _, Py, PyAny, PyResult, Python};

/// This is a helper function to initialize the required pyarrow data
/// types for passing into `datafusion.udf()`
fn data_type_to_pyarrow_obj<'py>(
    pa: &Bound<'py, PyModule>,
    data_type: &DataType,
) -> PyResult<Bound<'py, PyAny>> {
    match data_type {
        DataType::Null => pa.getattr("utf8")?.call0(),
        DataType::Boolean => pa.getattr("bool_")?.call0(),
        DataType::Int8 => pa.getattr("int8")?.call0(),
        DataType::Int16 => pa.getattr("int16")?.call0(),
        DataType::Int32 => pa.getattr("int32")?.call0(),
        DataType::Int64 => pa.getattr("int64")?.call0(),
        DataType::UInt8 => pa.getattr("uint8")?.call0(),
        DataType::UInt16 => pa.getattr("uint16")?.call0(),
        DataType::UInt32 => pa.getattr("uint32")?.call0(),
        DataType::UInt64 => pa.getattr("uint64")?.call0(),
        DataType::Float16 => pa.getattr("float16")?.call0(),
        DataType::Float32 => pa.getattr("float32")?.call0(),
        DataType::Float64 => pa.getattr("float64")?.call0(),
        DataType::Date32 => pa.getattr("date32")?.call0(),
        DataType::Date64 => pa.getattr("date64")?.call0(),
        DataType::Binary => pa.getattr("binary")?.call0(),
        DataType::LargeBinary => pa.getattr("large_binary")?.call0(),
        DataType::BinaryView => pa.getattr("binary_view")?.call0(),
        DataType::Utf8 => pa.getattr("string")?.call0(),
        DataType::LargeUtf8 => pa.getattr("large_string")?.call0(),
        DataType::Utf8View => pa.getattr("string_view")?.call0(),

        DataType::FixedSizeBinary(_)
        | DataType::Timestamp(_, _)
        | DataType::Time32(_)
        | DataType::Time64(_)
        | DataType::Duration(_)
        | DataType::Interval(_)
        | DataType::List(_)
        | DataType::ListView(_)
        | DataType::FixedSizeList(_, _)
        | DataType::LargeList(_)
        | DataType::LargeListView(_)
        | DataType::Struct(_)
        | DataType::Union(_, _)
        | DataType::Dictionary(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _)
        | DataType::Map(_, _)
        | DataType::RunEndEncoded(_, _) => {
            Err(PyRuntimeError::new_err("Data type is not supported"))
        }
    }
}

/// This helper function will take a closure and turn it into a `DataFusion` scalar UDF.
/// It calls the python `datafusion.udf()` function. These may get removed once
/// <https://github.com/apache/datafusion/issues/14562> and the associated support
/// in `datafusion-python` are completed.
pub fn create_datafusion_scalar_udf<F>(
    py: Python<'_>,
    closure: F,
    arg_types: &[&DataType],
    return_type: &DataType,
    volatility: Volatility,
) -> PyResult<Py<PyAny>>
where
    F: Fn(&Bound<'_, PyTuple>, Option<&Bound<'_, PyDict>>) -> PyResult<Py<PyAny>> + Send + 'static,
{
    let udf_factory = py
        .import("datafusion")
        .and_then(|datafusion| datafusion.getattr("udf"))?;
    let pyarrow_module = py.import("pyarrow")?;
    let arg_types = arg_types
        .iter()
        .map(|arg_type| data_type_to_pyarrow_obj(&pyarrow_module, arg_type))
        .collect::<PyResult<Vec<_>>>()?;

    let arg_types = PyList::new(py, arg_types)?;
    let return_type = data_type_to_pyarrow_obj(&pyarrow_module, return_type)?;

    let inner = PyCFunction::new_closure(py, None, None, closure)?;
    let bound_inner = inner.into_pyobject(py)?;

    let volatility = match volatility {
        Volatility::Immutable => "immutable",
        Volatility::Stable => "stable",
        Volatility::Volatile => "volatile",
    };
    let py_stable = PyString::new(py, volatility);

    let args = PyTuple::new(
        py,
        vec![
            bound_inner.as_any(),
            arg_types.as_any(),
            return_type.as_any(),
            py_stable.as_any(),
        ],
    )?;

    Ok(udf_factory.call1(args)?.unbind())
}
