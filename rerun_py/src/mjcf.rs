use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::ArrayData;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use re_sdk::EntityPath;
use re_sdk::external::re_importer::{
    KinematicTree as _, MjcfBody, MjcfJoint, MjcfJointType, MjcfTree,
};

use crate::chunk_stream::PyLazyChunkStreamInternal;
use crate::kinematic_tree;
use crate::python_bridge::PyRecordingStream;

/// An MJCF model loaded into memory (excluding any mesh files).
#[pyclass(name = "_MjcfTreeInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PyMjcfTree(Arc<MjcfTree>);

#[pymethods]
impl PyMjcfTree {
    /// Load the MJCF model found at `path`, along with the files it includes.
    #[staticmethod]
    #[pyo3(signature = (path, entity_path_prefix=None, *, frame_prefix=None, static_transform_entity_path=None))]
    #[pyo3(
        text_signature = "(path, entity_path_prefix=None, *, frame_prefix=None, static_transform_entity_path=None)"
    )]
    pub fn from_file_path(
        path: PathBuf,
        entity_path_prefix: Option<String>,
        frame_prefix: Option<String>,
        static_transform_entity_path: Option<String>,
    ) -> PyResult<Self> {
        let mut tree = MjcfTree::from_file_path(path, entity_path_prefix.map(EntityPath::from))
            .map_err(|err| PyRuntimeError::new_err(format!("{err:#}")))?;
        if let Some(prefix) = frame_prefix {
            tree = tree.with_frame_prefix(prefix);
        }
        if let Some(entity_path) = static_transform_entity_path {
            tree = tree.with_static_transform_entity(entity_path);
        }
        Ok(Self(Arc::new(tree)))
    }

    /// Name of the model.
    #[getter]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// The frame prefix, if set.
    #[getter]
    pub fn frame_prefix(&self) -> Option<&str> {
        self.0.frame_prefix()
    }

    /// Returns the world body, which is the root of the body tree.
    pub fn root_body(&self) -> PyResult<PyMjcfBody> {
        self.0
            .bodies()
            .first()
            .map(|body| PyMjcfBody::new(&self.0, body))
            .ok_or_else(|| {
                PyRuntimeError::new_err(format!("MJCF model {:?} has no world body", self.0.name()))
            })
    }

    /// All bodies, parents before children.
    pub fn bodies(&self) -> Vec<PyMjcfBody> {
        self.0
            .bodies()
            .iter()
            .map(|body| PyMjcfBody::new(&self.0, body))
            .collect()
    }

    /// Find a body by name.
    pub fn get_body_by_name(&self, body_name: &str) -> Option<PyMjcfBody> {
        self.0
            .get_body_by_name(body_name)
            .map(|body| PyMjcfBody::new(&self.0, body))
    }

    /// All joints, in the order of their bodies.
    pub fn joints(&self) -> Vec<PyMjcfJoint> {
        self.0
            .joints()
            .iter()
            .map(|joint| PyMjcfJoint::new(&self.0, joint))
            .collect()
    }

    /// Find a joint by name.
    pub fn get_joint_by_name(&self, joint_name: &str) -> Option<PyMjcfJoint> {
        self.0
            .get_joint_by_name(joint_name)
            .map(|joint| PyMjcfJoint::new(&self.0, joint))
    }

    /// Returns the entity paths for all visual geometries of the given body, or none if it is not
    /// part of this model.
    pub fn get_visual_geometry_paths(&self, body: &PyMjcfBody) -> Vec<String> {
        self.0
            .get_visual_geometry_paths(&body.body)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Returns the entity paths for all collision geometries of the given body, or none if it is not
    /// part of this model.
    pub fn get_collision_geometry_paths(&self, body: &PyMjcfBody) -> Vec<String> {
        self.0
            .get_collision_geometry_paths(&body.body)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Log the full model (geometry + static body transforms) to a recording stream.
    ///
    /// Frame IDs respect the tree's `frame_prefix` if set.
    #[pyo3(signature = (recording=None))]
    pub(crate) fn log(&self, recording: Option<&PyRecordingStream>) -> PyResult<()> {
        kinematic_tree::log(self.0.as_ref(), recording)
    }

    /// Return a new lazy stream over all chunks emitted from this MJCF tree.
    #[pyo3(signature = (*, include_joint_transforms = true))]
    pub(crate) fn stream(&self, include_joint_transforms: bool) -> PyLazyChunkStreamInternal {
        kinematic_tree::stream(Arc::clone(&self.0) as _, include_joint_transforms)
    }

    /// Compute transform batches from per-row joint name and value arrays.
    #[pyo3(signature = (names, values, *, clamp = false))]
    pub fn compute_joint_transform_batches(
        &self,
        py: Python<'_>,
        names: PyArrowType<ArrayData>,
        values: PyArrowType<ArrayData>,
        clamp: bool,
    ) -> PyResult<Py<PyAny>> {
        kinematic_tree::compute_joint_transform_batches(py, self.0.as_ref(), names, values, clamp)
    }

    fn __repr__(&self) -> String {
        format!("MjcfTree(name={:?})", self.0.name())
    }
}

/// Wrapper around an MJCF joint.
#[pyclass(
    name = "_MjcfJointInternal",
    from_py_object,
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyMjcfJoint {
    tree: Arc<MjcfTree>,
    joint: MjcfJoint,
}

impl PyMjcfJoint {
    fn new(tree: &Arc<MjcfTree>, joint: &MjcfJoint) -> Self {
        Self {
            tree: Arc::clone(tree),
            joint: joint.clone(),
        }
    }

    /// The body this joint moves.
    fn joint_body(&self) -> PyResult<&MjcfBody> {
        self.tree.get_joint_body(&self.joint).ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Joint {:?} is not part of MJCF model {:?}",
                self.joint.name,
                self.tree.name()
            ))
        })
    }
}

#[pymethods]
impl PyMjcfJoint {
    /// Name of the joint.
    #[getter]
    pub fn name(&self) -> &str {
        &self.joint.name
    }

    /// Type of the joint: `hinge`, `slide`, `ball` or `free`.
    #[getter]
    pub fn joint_type(&self) -> &'static str {
        joint_type_name(self.joint.joint_type)
    }

    /// Name of the body this joint moves.
    #[getter]
    pub fn body(&self) -> PyResult<&str> {
        Ok(&self.joint_body()?.name)
    }

    /// Name of the parent of the body this joint moves.
    #[getter]
    pub fn parent_body(&self) -> PyResult<Option<&str>> {
        Ok(self
            .tree
            .get_parent_body(self.joint_body()?)
            .map(|parent| parent.name.as_str()))
    }

    /// Unit axis of the joint, in the body's frame.
    #[getter]
    pub fn axis(&self) -> (f64, f64, f64) {
        self.joint.axis.into()
    }

    /// The point the joint rotates about, in the body's frame.
    #[getter]
    pub fn anchor(&self) -> (f64, f64, f64) {
        self.joint.anchor.into()
    }

    /// The value at which the body is in its reference pose (MJCF `ref`).
    #[getter]
    pub fn reference(&self) -> f64 {
        self.joint.reference
    }

    /// The lower and upper limit, or `None` if the joint is not limited.
    #[getter]
    pub fn range(&self) -> Option<(f64, f64)> {
        self.joint
            .range
            .as_ref()
            .map(|range| (*range.start(), *range.end()))
    }

    /// Compute the transform components for this joint at the given value.
    ///
    /// Returns a dict, see [`kinematic_tree::joint_transform_dict`].
    ///
    /// If `clamp` is true, values outside joint limits will be clamped and a warning is generated.
    /// If `clamp` is false (default), values outside limits are used as-is without warnings.
    #[pyo3(signature = (value, clamp = false))]
    pub fn compute_transform(
        &self,
        py: Python<'_>,
        value: f64,
        clamp: bool,
    ) -> PyResult<Py<PyAny>> {
        let transform = self
            .tree
            .compute_joint_transform(&self.joint, value, clamp)
            .map_err(|err| kinematic_tree::to_py_err(&err))?;
        kinematic_tree::joint_transform_dict(py, transform)
    }

    /// Compute transforms for this joint at multiple values in a single call.
    ///
    /// Returns a dict, see [`kinematic_tree::joint_transform_columns`].
    #[pyo3(signature = (values, *, clamp = false))]
    #[allow(clippy::needless_pass_by_value)] // PyO3 requires owned Vec for Python list extraction
    pub fn compute_transform_columns(
        &self,
        py: Python<'_>,
        values: Vec<f64>,
        clamp: bool,
    ) -> PyResult<Py<PyAny>> {
        kinematic_tree::joint_transform_columns(py, &values, |value| {
            self.tree.compute_joint_transform(&self.joint, value, clamp)
        })
    }

    fn __repr__(&self) -> String {
        let joint = &self.joint;
        let body = self.joint_body().map_or("?", |body| body.name.as_str());
        format!(
            "MjcfJoint(name={:?}, type={}, body={body:?})",
            joint.name,
            joint_type_name(joint.joint_type),
        )
    }
}

/// The MJCF `type` attribute value.
fn joint_type_name(joint_type: MjcfJointType) -> &'static str {
    match joint_type {
        MjcfJointType::Hinge => "hinge",
        MjcfJointType::Slide => "slide",
        MjcfJointType::Ball => "ball",
        MjcfJointType::Free => "free",
    }
}

/// MJCF body
#[pyclass(
    name = "_MjcfBodyInternal",
    from_py_object,
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyMjcfBody {
    tree: Arc<MjcfTree>,
    body: MjcfBody,
}

impl PyMjcfBody {
    fn new(tree: &Arc<MjcfTree>, body: &MjcfBody) -> Self {
        Self {
            tree: Arc::clone(tree),
            body: body.clone(),
        }
    }
}

#[pymethods]
impl PyMjcfBody {
    /// Name of the body, which is also its frame ID (before applying any frame prefix).
    #[getter]
    pub fn name(&self) -> &str {
        &self.body.name
    }

    /// Name of the parent body, or `None` for the world body.
    #[getter]
    pub fn parent(&self) -> Option<&str> {
        self.tree
            .get_parent_body(&self.body)
            .map(|parent| parent.name.as_str())
    }

    /// Translation relative to the parent body, when all joints are at their reference value.
    #[getter]
    pub fn translation(&self) -> (f64, f64, f64) {
        self.body.pose.translation.into()
    }

    /// Rotation relative to the parent body as a unit quaternion in `xyzw` order (MuJoCo uses `wxyz`),
    /// when all joints are at their reference value.
    #[getter]
    pub fn quaternion_xyzw(&self) -> (f64, f64, f64, f64) {
        self.body.pose.rotation.into()
    }

    /// The joints that move this body relative to its parent, in the order MuJoCo applies them.
    pub fn joints(&self) -> PyResult<Vec<PyMjcfJoint>> {
        self.body
            .joints
            .iter()
            .map(|&id| {
                let joint = self.tree.get_joint(id).ok_or_else(|| {
                    PyRuntimeError::new_err(format!(
                        "Body {:?} refers to a joint that is not part of MJCF model {:?}",
                        self.body.name,
                        self.tree.name()
                    ))
                })?;
                Ok(PyMjcfJoint::new(&self.tree, joint))
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!("MjcfBody(name={:?})", self.name())
    }
}

/// Register the `rerun.mjcf` module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMjcfTree>()?;
    m.add_class::<PyMjcfJoint>()?;
    m.add_class::<PyMjcfBody>()?;

    Ok(())
}
