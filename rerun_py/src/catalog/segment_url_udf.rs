use std::any::Any;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};

use arrow::array::{Array as _, ArrayRef, AsArray as _, StringBuilder};
use arrow::compute::{can_cast_types, cast};
use arrow::datatypes::DataType;
use datafusion::common::Result as DataFusionResult;
use datafusion::error::DataFusionError;
use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl, Signature, Volatility,
};
use datafusion_ffi::udf::FFI_ScalarUDF;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyResult, Python, pyclass, pymethods};

use re_log_types::{NonMinI64, TimeCell, TimeType, TimelineName};
use re_tuid::Tuid;
use re_types_core::Loggable as _;
use re_uri::{DatasetSegmentUri, Fragment, Origin};

#[derive(Debug)]
struct SegmentUrlUdf {
    signature: Signature,
}

impl PartialEq for SegmentUrlUdf {
    fn eq(&self, _other: &Self) -> bool {
        // reminder to update this when new fields are added
        let Self { signature: _ } = self;

        true
    }
}

impl Eq for SegmentUrlUdf {}

impl Hash for SegmentUrlUdf {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // reminder to update this when new fields are added
        let Self { signature: _ } = self;
    }
}

impl SegmentUrlUdf {
    fn new() -> Self {
        Self {
            // It is difficult to express the signature of this udf with the `Signature` struct.
            // The reason for that is that we have optional features (and may have more in the
            // future), as well as some columns that accepts multiple types. This would lead to
            // combinatorial explosion when using `one_of`.
            //
            // Instead, we:
            // - check the input type in `Self::return_type` (still give use plan-time validation)
            // - handle casting ourselves (we don't get automatic coercion when using `any`)
            signature: Signature::any(5, Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for SegmentUrlUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "segment_url"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, arg_types: &[DataType]) -> DataFusionResult<DataType> {
        // validate signature
        if arg_types.len() != 5 {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects 5 arguments, got {}",
                arg_types.len()
            )));
        }

        // arg 0: origin (castable to Utf8)
        if !can_cast_types(&arg_types[0], &DataType::Utf8) {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects origin (arg 0) to be castable to Utf8, got {}",
                arg_types[0]
            )));
        }

        // arg 1: entry_id (FixedSizeBinary(16))
        if arg_types[1] != DataType::FixedSizeBinary(16) {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects entry_id (arg 1) to be FixedSizeBinary(16), got {}",
                arg_types[1]
            )));
        }

        // arg 2: segment_id (castable to Utf8)
        if !can_cast_types(&arg_types[2], &DataType::Utf8) {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects segment_id (arg 2) to be castable to Utf8, got {}",
                arg_types[2]
            )));
        }

        // arg 3: timestamp
        let time_type = TimeType::from_arrow_datatype(&arg_types[3]);
        if time_type.is_none() && arg_types[3] != DataType::Null {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects timestamp (arg 3) to be either null, or a supported timestamp datatype, got {}",
                arg_types[3]
            )));
        }

        // arg 4: timeline (scalar only)
        if !(arg_types[4] == DataType::Null || can_cast_types(&arg_types[4], &DataType::Utf8)) {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects timeline (arg 4) to be castable to Utf8, got {}",
                arg_types[4]
            )));
        }

        // timeline must be provided if timestamp is not null
        if arg_types[3] != DataType::Null && arg_types[4] == DataType::Null {
            return Err(DataFusionError::Plan(format!(
                "segment_url_udf expects timeline (arg 4) to be provided if timestamp (arg 3) is not null, got {}",
                arg_types[4]
            )));
        }

        Ok(DataType::Utf8)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DataFusionResult<ColumnarValue> {
        // Derive num_rows from all arguments: each is either a scalar (length 1) or an array (length N).
        let num_rows = {
            let mut n: Option<usize> = None;
            for (i, arg) in args.args.iter().enumerate() {
                if let ColumnarValue::Array(arr) = arg {
                    let len = arr.len();
                    if len != 1 {
                        if let Some(n) = n {
                            if n != len {
                                return Err(DataFusionError::Execution(format!(
                                    "segment_url: argument {i} has {len} rows, expected 1 or {n}"
                                )));
                            }
                        } else {
                            n = Some(len);
                        }
                    }
                }
            }
            n.unwrap_or(1)
        };

        // Arg 0: origin (scalar)
        let origin_str = extract_scalar_string(&args.args[0])?;
        let origin: Origin = origin_str.parse().map_err(|err| {
            DataFusionError::Execution(format!("segment_url: failed to parse origin: {err}"))
        })?;

        // Arg 1: entry_id (scalar)
        let dataset_id = extract_scalar_entry_id(&args.args[1])?;

        // Arg 2: segment_id — cast to Utf8 to handle Utf8View/LargeUtf8
        let segment_id_array = cast_array(&args.args[2].to_array(num_rows)?, &DataType::Utf8)?;
        let segment_ids = segment_id_array.as_string::<i32>();

        // Arg 3: timestamp, Arg 4: timeline
        let ts_datatype = args.args[3].data_type();
        let time_info = if ts_datatype == DataType::Null {
            None
        } else {
            let timeline_name = extract_scalar_string(&args.args[4])?;

            let time_type = TimeType::from_arrow_datatype(&ts_datatype).ok_or_else(|| {
                DataFusionError::Execution(format!(
                    "segment_url: unsupported timestamp datatype {ts_datatype:?}"
                ))
            })?;

            let ts_array = args.args[3].to_array(num_rows)?;
            let ts_array = cast_array(&ts_array, &DataType::Int64)?;

            Some((timeline_name, time_type, ts_array))
        };

        let mut string_builder = StringBuilder::new();

        for row in 0..num_rows {
            if segment_ids.is_null(row) {
                string_builder.append_null();
                continue;
            }

            let segment_id = segment_ids.value(row).to_owned();

            let when = time_info
                .as_ref()
                .and_then(|(timeline, time_type, ts_array)| {
                    if ts_array.is_null(row) {
                        return None;
                    }
                    let i64_val = ts_array
                        .as_primitive::<arrow::datatypes::Int64Type>()
                        .value(row);
                    let time_cell = TimeCell::new(*time_type, NonMinI64::try_from(i64_val).ok()?);
                    Some((TimelineName::new(timeline.as_str()), time_cell))
                });

            // TODO(ab): this is an unfortunate lot of cloning just to format a URL string, but
            // chances are we'll run in other problems by the time this becomes a performance issue.
            let uri = DatasetSegmentUri {
                origin: origin.clone(),
                dataset_id,
                segment_id,
                fragment: Fragment {
                    selection: None,
                    when,
                    time_selection: None,
                },
            };

            string_builder.append_value(uri.to_string());
        }

        let array: ArrayRef = Arc::new(string_builder.finish());
        Ok(ColumnarValue::Array(array))
    }
}

/// Extract a single string value from a [`ColumnarValue`].
///
/// Handles both `Scalar` and `Array` (takes the first non-null value) because the
/// DataFusion FFI layer may expand scalars into single-element arrays.
fn extract_scalar_string(col: &ColumnarValue) -> DataFusionResult<String> {
    match col {
        ColumnarValue::Scalar(scalar) => Ok(scalar.to_string()),

        ColumnarValue::Array(array) => {
            let array = cast_array(array, &DataType::Utf8)?;
            let arr = array.as_string::<i32>();
            arr.iter()
                .flatten()
                .next()
                .ok_or_else(|| {
                    DataFusionError::Execution(
                        "segment_url: expected a non-null scalar string value".to_owned(),
                    )
                })
                .map(|s| s.to_owned())
        }
    }
}

/// Extract a single `Tuid` from a [`ColumnarValue`] of `FixedSizeBinary(16)`.
///
/// Handles both `Scalar` and `Array` (takes the first non-null value) because the
/// DataFusion FFI layer may expand scalars into single-element arrays.
///
/// Delegates to [`Tuid::from_arrow`] for deserialization.
fn extract_scalar_entry_id(col: &ColumnarValue) -> DataFusionResult<Tuid> {
    // Convert to array to handle both Scalar and Array uniformly.
    let array = col.to_array(1)?;

    let tuids = Tuid::from_arrow(array.as_ref()).map_err(|err| {
        DataFusionError::Execution(format!(
            "segment_url: failed to deserialize entry_id as Tuid: {err}"
        ))
    })?;

    tuids.into_iter().next().ok_or_else(|| {
        DataFusionError::Execution(
            "segment_url: entry_id (arg 1) must be a non-null value".to_owned(),
        )
    })
}

fn cast_array(array: &ArrayRef, target_datatype: &DataType) -> DataFusionResult<ArrayRef> {
    cast(array, target_datatype).map_err(|err| {
        DataFusionError::Execution(format!(
            "segment_url: failed to cast array of type {} to {target_datatype}: {err}",
            array.data_type(),
        ))
    })
}

/// Global singleton UDF instance — all users share the same stateless UDF.
static SEGMENT_URL_UDF: LazyLock<Arc<ScalarUDF>> =
    LazyLock::new(|| Arc::new(ScalarUDF::new_from_impl(SegmentUrlUdf::new())));

#[pyclass( // NOLINT: ignore[py-cls-eq] internal class
    frozen,
    name = "SegmentUrlUdfInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PySegmentUrlUdfInternal {
    udf: Arc<ScalarUDF>,
}

#[pymethods] // NOLINT: ignore[py-mthd-str] internal class
impl PySegmentUrlUdfInternal {
    #[new]
    fn py_new() -> Self {
        Self {
            udf: Arc::clone(&SEGMENT_URL_UDF),
        }
    }

    /// Scalar UDF pycapsule.
    fn __datafusion_scalar_udf__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let ffi_udf = FFI_ScalarUDF::from(Arc::clone(&self.udf));
        let capsule_name = cr"datafusion_scalar_udf".into();
        PyCapsule::new(py, ffi_udf, Some(capsule_name))
    }
}
