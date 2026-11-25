//! Format-independent binding helpers for robot models, used by `rerun.urdf`.

use std::sync::Arc;

use arrow::array::{Array as _, ArrayData, ListArray, make_array};
use arrow::pyarrow::{PyArrowType, ToPyArrow as _};
use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sdk::TimePoint;
use re_sdk::external::re_importer::{JointTransform, KinematicTree, UnsupportedJointType};

use crate::chunk_stream::PyLazyChunkStreamInternal;
use crate::chunk_stream::kinematic_tree_stream::KinematicTreeStreamFactory;
use crate::chunk_stream::stream::LazyChunkStream;
use crate::python_bridge::{PyRecordingStream, get_data_recording};

/// Raises `NotImplementedError` for joints that cannot be posed, and `ValueError` otherwise.
pub fn to_py_err(err: &anyhow::Error) -> PyErr {
    if err.is::<UnsupportedJointType>() {
        PyNotImplementedError::new_err(err.to_string())
    } else {
        PyValueError::new_err(format!("{err:#}"))
    }
}

/// Logs the full model (geometry and static joint transforms) to `recording`, or the active recording.
pub fn log(tree: &dyn KinematicTree, recording: Option<&PyRecordingStream>) -> PyResult<()> {
    let Some(recording) = get_data_recording(recording) else {
        return Ok(());
    };

    let mut chunks = Vec::new();
    tree.emit(&mut |chunk| chunks.push(chunk), &TimePoint::STATIC, true)
        .map_err(|err| {
            PyRuntimeError::new_err(format!("Failed to log {}: {err:#}", tree.model_format()))
        })?;

    recording.send_chunks(chunks);

    Ok(())
}

/// A lazy stream over all chunks emitted from `tree`.
pub fn stream(
    tree: Arc<dyn KinematicTree>,
    include_joint_transforms: bool,
) -> PyLazyChunkStreamInternal {
    PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(
        KinematicTreeStreamFactory::new(tree, include_joint_transforms),
    ))
}

/// One transform as a dict with `quaternion_xyzw`, `translation`, `parent_frame`, `child_frame` and `warnings`.
pub fn joint_transform_dict(py: Python<'_>, transform: JointTransform) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("quaternion_xyzw", transform.quaternion.to_array())?;
    dict.set_item("translation", transform.translation.to_array())?;
    dict.set_item("parent_frame", transform.parent_frame)?;
    dict.set_item("child_frame", transform.child_frame)?;
    dict.set_item("warnings", transform.warnings)?;
    Ok(dict.into())
}

/// The transforms of one joint at each of `values`, as columns.
///
/// Returns a dict with `translations`, `quaternions_xyzw`, `warnings`, and the `parent_frame` and
/// `child_frame` shared by all transforms (empty if there are no values).
pub fn joint_transform_columns(
    py: Python<'_>,
    values: &[f64],
    mut compute: impl FnMut(f64) -> anyhow::Result<JointTransform>,
) -> PyResult<Py<PyAny>> {
    let mut translations = Vec::with_capacity(values.len());
    let mut quaternions = Vec::with_capacity(values.len());
    let mut warnings = Vec::new();
    let mut frames = None;

    for &value in values {
        let transform = compute(value).map_err(|err| to_py_err(&err))?;
        translations.push(transform.translation.to_array());
        quaternions.push(transform.quaternion.to_array());
        warnings.extend(transform.warnings);
        frames.get_or_insert((transform.parent_frame, transform.child_frame));
    }
    let (parent_frame, child_frame) = frames.unwrap_or_default();

    let dict = PyDict::new(py);
    dict.set_item("translations", translations)?;
    dict.set_item("quaternions_xyzw", quaternions)?;
    dict.set_item("parent_frame", parent_frame)?;
    dict.set_item("child_frame", child_frame)?;
    dict.set_item("warnings", warnings)?;
    Ok(dict.into())
}

/// Computes transform batches from per-row joint name and value arrays, see [`KinematicTree::compute_joint_transform_batches`].
pub fn compute_joint_transform_batches(
    py: Python<'_>,
    tree: &dyn KinematicTree,
    names: PyArrowType<ArrayData>,
    values: PyArrowType<ArrayData>,
    clamp: bool,
) -> PyResult<Py<PyAny>> {
    let names = make_array(names.0);
    let values = make_array(values.0);

    let names = names
        .try_downcast_array_ref::<ListArray>()
        .map_err(|err| PyValueError::new_err(format!("joint names: {err}")))?;
    let values = values
        .try_downcast_array_ref::<ListArray>()
        .map_err(|err| PyValueError::new_err(format!("joint values: {err}")))?;

    let result = tree
        .compute_joint_transform_batches(names, values, clamp)
        .map_err(|err| to_py_err(&err))?;

    result.to_data().to_pyarrow(py).map(|obj| obj.unbind())
}
