use std::path::PathBuf;

use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError};
use pyo3::prelude::*;
use re_sdk::external::re_data_loader::{UrdfTree, urdf_joint_transform};
use re_sdk::external::urdf_rs::{Joint, JointType, Link};
use re_sdk::{EntityPath, TimePoint};

use crate::python_bridge::{PyRecordingStream, get_data_recording};

/// A `.urdf` file loaded into memory (excluding any mesh files).
#[pyclass(name = "_UrdfTreeInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PyUrdfTree(UrdfTree);

#[pymethods]
impl PyUrdfTree {
    /// Load the URDF found at `path`.
    #[staticmethod]
    #[pyo3(text_signature = "(path, entity_path_prefix=None, frame_prefix=None)")]
    pub fn from_file_path(
        path: PathBuf,
        entity_path_prefix: Option<String>,
        frame_prefix: Option<String>,
    ) -> PyResult<Self> {
        let mut tree =
            UrdfTree::from_file_path(path, entity_path_prefix.map(EntityPath::from_single_string))
                .map_err(|err| {
                    PyRuntimeError::new_err(format!("Failed to load URDF file: {err}"))
                })?;
        if let Some(prefix) = frame_prefix {
            tree = tree.with_frame_prefix(prefix);
        }
        Ok(Self(tree))
    }

    /// Name of the robot defined in this URDF.
    #[getter]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// The frame prefix, if set.
    #[getter]
    pub fn frame_prefix(&self) -> Option<&str> {
        self.0.frame_prefix()
    }

    /// Returns the root link of the URDF hierarchy.
    pub fn root_link(&self) -> PyUrdfLink {
        PyUrdfLink(self.0.root().clone())
    }

    /// Iterate over all joints defined in the URDF.
    pub fn joints(&self) -> Vec<PyUrdfJoint> {
        let frame_prefix = self.0.frame_prefix().map(str::to_owned);
        self.0
            .joints()
            .cloned()
            .map(|j| PyUrdfJoint {
                joint: j,
                frame_prefix: frame_prefix.clone(),
            })
            .collect()
    }

    /// Find a joint by name.
    pub fn get_joint_by_name(&self, joint_name: &str) -> Option<PyUrdfJoint> {
        self.0
            .get_joint_by_name(joint_name)
            .cloned()
            .map(|j| PyUrdfJoint {
                joint: j,
                frame_prefix: self.0.frame_prefix().map(str::to_owned),
            })
    }

    /// Returns the link that is the child of the given joint.
    pub fn get_joint_child(&self, joint: &PyUrdfJoint) -> PyUrdfLink {
        PyUrdfLink(self.0.get_joint_child(&joint.joint).clone())
    }

    /// Returns the link with the given name, if it exists.
    pub fn get_link_by_name(&self, link_name: &str) -> Option<PyUrdfLink> {
        self.0.get_link(link_name).cloned().map(PyUrdfLink)
    }

    /// Returns the entity paths for all visual geometries of the given link, if any.
    pub fn get_visual_geometry_paths(&self, link: &PyUrdfLink) -> Vec<String> {
        self.0
            .get_visual_geometries(&link.0)
            .unwrap_or_default()
            .into_iter()
            .map(|(entity_path, _)| entity_path.to_string())
            .collect()
    }

    /// Returns the entity paths for all collision geometries of the given link, if any.
    pub fn get_collision_geometry_paths(&self, link: &PyUrdfLink) -> Vec<String> {
        self.0
            .get_collision_geometries(&link.0)
            .unwrap_or_default()
            .into_iter()
            .map(|(entity_path, _)| entity_path.to_string())
            .collect()
    }

    /// Log the full robot model (geometry + static transforms) to a recording stream.
    ///
    /// Frame IDs respect the tree's `frame_prefix` if set.
    #[pyo3(signature = (recording=None))]
    pub(crate) fn log(&self, recording: Option<&PyRecordingStream>) -> PyResult<()> {
        let Some(recording) = get_data_recording(recording) else {
            return Ok(());
        };

        let mut chunks = Vec::new();
        self.0
            .emit(&mut |chunk| chunks.push(chunk), &TimePoint::default(), true)
            .map_err(|err| PyRuntimeError::new_err(format!("Failed to log URDF: {err}")))?;

        recording.send_chunks(chunks);

        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("UrdfTree(name={:?})", self.0.name())
    }
}

/// Wrapper around a URDF joint.
#[pyclass(name = "_UrdfJointInternal", module = "rerun_bindings.rerun_bindings")]
#[derive(Clone)]
pub struct PyUrdfJoint {
    pub joint: Joint,
    pub frame_prefix: Option<String>,
}

#[pymethods]
impl PyUrdfJoint {
    /// Name of the joint.
    #[getter]
    pub fn name(&self) -> &str {
        &self.joint.name
    }

    /// Type of the joint.
    #[getter]
    pub fn joint_type(&self) -> &'static str {
        match self.joint.joint_type {
            JointType::Revolute => "revolute",
            JointType::Continuous => "continuous",
            JointType::Prismatic => "prismatic",
            JointType::Fixed => "fixed",
            JointType::Floating => "floating",
            JointType::Planar => "planar",
            JointType::Spherical => "spherical",
        }
    }

    /// Name of the parent link.
    #[getter]
    pub fn parent_link(&self) -> &str {
        &self.joint.parent.link
    }

    /// Name of the child link.
    #[getter]
    pub fn child_link(&self) -> &str {
        &self.joint.child.link
    }

    /// Axis of the joint.
    #[getter]
    pub fn axis(&self) -> (f64, f64, f64) {
        self.joint.axis.xyz.0.into()
    }

    /// Origin of the joint (translation).
    #[getter]
    pub fn origin_xyz(&self) -> (f64, f64, f64) {
        self.joint.origin.xyz.0.into()
    }

    /// Origin of the joint (rotation in roll, pitch, yaw).
    #[getter]
    pub fn origin_rpy(&self) -> (f64, f64, f64) {
        self.joint.origin.rpy.0.into()
    }

    /// Lower limit of the joint.
    #[getter]
    pub fn limit_lower(&self) -> f64 {
        self.joint.limit.lower
    }

    /// Upper limit of the joint.
    #[getter]
    pub fn limit_upper(&self) -> f64 {
        self.joint.limit.upper
    }

    /// Effort limit of the joint.
    #[getter]
    pub fn limit_effort(&self) -> f64 {
        self.joint.limit.effort
    }

    /// Velocity limit of the joint.
    #[getter]
    pub fn limit_velocity(&self) -> f64 {
        self.joint.limit.velocity
    }

    /// Compute the transform components for this joint at the given value.
    ///
    /// The result is wrapped in a dictionary for easy conversion to the final types in Python.
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
        match urdf_joint_transform::internal::compute_joint_transform(&self.joint, value, clamp) {
            Ok(result) => {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item(
                    "quaternion_xyzw",
                    (
                        result.quaternion.x,
                        result.quaternion.y,
                        result.quaternion.z,
                        result.quaternion.w,
                    ),
                )?;
                dict.set_item(
                    "translation",
                    (
                        result.translation.x,
                        result.translation.y,
                        result.translation.z,
                    ),
                )?;
                dict.set_item(
                    "parent_frame",
                    Self::apply_prefix(&self.frame_prefix, &result.parent_frame),
                )?;
                dict.set_item(
                    "child_frame",
                    Self::apply_prefix(&self.frame_prefix, &result.child_frame),
                )?;
                dict.set_item("warning", result.warning)?;

                Ok(dict.into())
            }
            Err(e @ urdf_joint_transform::Error::UnsupportedJointType(_)) => {
                Err(PyNotImplementedError::new_err(e.to_string()))
            }
        }
    }

    /// Compute transforms for this joint at multiple values in a single call.
    ///
    /// Returns a dictionary with:
    /// - `"translations"`: list of `(x, y, z)` tuples
    /// - `"quaternions_xyzw"`: list of `(x, y, z, w)` tuples
    /// - `"parent_frame"`: single string (constant per joint)
    /// - `"child_frame"`: single string (constant per joint)
    /// - `"warnings"`: list of warning strings
    #[pyo3(signature = (values, *, clamp = false))]
    #[allow(clippy::needless_pass_by_value)] // PyO3 requires owned Vec for Python list extraction
    pub fn compute_transform_columns(
        &self,
        py: Python<'_>,
        values: Vec<f64>,
        clamp: bool,
    ) -> PyResult<Py<PyAny>> {
        let mut translations = Vec::with_capacity(values.len());
        let mut quaternions = Vec::with_capacity(values.len());
        let mut warnings = Vec::new();
        let mut parent_frame = String::new();
        let mut child_frame = String::new();

        for (i, &value) in values.iter().enumerate() {
            match urdf_joint_transform::internal::compute_joint_transform(&self.joint, value, clamp)
            {
                Ok(result) => {
                    translations.push((
                        result.translation.x,
                        result.translation.y,
                        result.translation.z,
                    ));
                    quaternions.push((
                        result.quaternion.x,
                        result.quaternion.y,
                        result.quaternion.z,
                        result.quaternion.w,
                    ));
                    if let Some(warning) = result.warning {
                        warnings.push(warning);
                    }
                    if i == 0 {
                        parent_frame = Self::apply_prefix(&self.frame_prefix, &result.parent_frame);
                        child_frame = Self::apply_prefix(&self.frame_prefix, &result.child_frame);
                    }
                }
                Err(e @ urdf_joint_transform::Error::UnsupportedJointType(_)) => {
                    return Err(PyNotImplementedError::new_err(e.to_string()));
                }
            }
        }

        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("translations", translations)?;
        dict.set_item("quaternions_xyzw", quaternions)?;
        dict.set_item("parent_frame", parent_frame)?;
        dict.set_item("child_frame", child_frame)?;
        dict.set_item("warnings", warnings)?;

        Ok(dict.into())
    }

    fn __repr__(&self) -> String {
        format!(
            "UrdfJoint(name={:?}, type={}, parent={:?}, child={:?})",
            self.joint.name,
            &self.joint_type(),
            self.joint.parent.link,
            self.joint.child.link
        )
    }
}

impl PyUrdfJoint {
    fn apply_prefix(prefix: &Option<String>, frame_id: &str) -> String {
        match prefix {
            Some(prefix) => format!("{prefix}{frame_id}"),
            None => frame_id.to_owned(),
        }
    }
}

/// URDF link
#[pyclass(name = "_UrdfLinkInternal", module = "rerun_bindings.rerun_bindings")]
#[derive(Clone)]
pub struct PyUrdfLink(pub Link);

#[pymethods]
impl PyUrdfLink {
    /// Name of the link.
    #[getter]
    pub fn name(&self) -> &str {
        &self.0.name
    }

    fn __repr__(&self) -> String {
        format!("UrdfLink(name={:?})", self.0.name)
    }
}

/// Register the `rerun.urdf` module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyUrdfTree>()?;
    m.add_class::<PyUrdfJoint>()?;
    m.add_class::<PyUrdfLink>()?;

    Ok(())
}
