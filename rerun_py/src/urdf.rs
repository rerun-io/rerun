use std::path::PathBuf;

use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError};
use pyo3::prelude::*;
use re_sdk::EntityPath;
use re_sdk::external::re_data_loader::{UrdfTree, urdf_joint_transform};
use re_sdk::external::urdf_rs::{Joint, JointType, Link};

/// A `.urdf` file loaded into memory (excluding any mesh files).
#[pyclass(name = "_UrdfTreeInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq], non-trivial implementation
pub struct PyUrdfTree(UrdfTree);

#[pymethods]
impl PyUrdfTree {
    /// Load the URDF found at `path`.
    #[staticmethod]
    #[pyo3(text_signature = "(path, entity_path_prefix=None)")]
    pub fn from_file_path(path: PathBuf, entity_path_prefix: Option<String>) -> PyResult<Self> {
        UrdfTree::from_file_path(path, entity_path_prefix.map(EntityPath::from_single_string))
            .map(Self)
            .map_err(|err| PyRuntimeError::new_err(format!("Failed to load URDF file: {err}")))
    }

    /// Name of the robot defined in this URDF.
    #[getter]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// Returns the root link of the URDF hierarchy.
    pub fn root_link(&self) -> PyUrdfLink {
        PyUrdfLink(self.0.root().clone())
    }

    /// Iterate over all joints defined in the URDF.
    pub fn joints(&self) -> Vec<PyUrdfJoint> {
        self.0.joints().cloned().map(PyUrdfJoint).collect()
    }

    /// Find a joint by name.
    pub fn get_joint_by_name(&self, joint_name: &str) -> Option<PyUrdfJoint> {
        self.0
            .get_joint_by_name(joint_name)
            .cloned()
            .map(PyUrdfJoint)
    }

    /// Returns the link that is the child of the given joint.
    pub fn get_joint_child(&self, joint: &PyUrdfJoint) -> PyUrdfLink {
        PyUrdfLink(self.0.get_joint_child(&joint.0).clone())
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

    fn __repr__(&self) -> String {
        format!("UrdfTree(name={:?})", self.0.name())
    }
}

/// Wrapper around a URDF joint.
#[pyclass(name = "_UrdfJointInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq], non-trivial implementation
#[derive(Clone)]
pub struct PyUrdfJoint(pub Joint);

#[pymethods]
impl PyUrdfJoint {
    /// Name of the joint.
    #[getter]
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// Type of the joint.
    #[getter]
    pub fn joint_type(&self) -> &'static str {
        match self.0.joint_type {
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
        &self.0.parent.link
    }

    /// Name of the child link.
    #[getter]
    pub fn child_link(&self) -> &str {
        &self.0.child.link
    }

    /// Axis of the joint.
    #[getter]
    pub fn axis(&self) -> (f64, f64, f64) {
        self.0.axis.xyz.0.into()
    }

    /// Origin of the joint (translation).
    #[getter]
    pub fn origin_xyz(&self) -> (f64, f64, f64) {
        self.0.origin.xyz.0.into()
    }

    /// Origin of the joint (rotation in roll, pitch, yaw).
    #[getter]
    pub fn origin_rpy(&self) -> (f64, f64, f64) {
        self.0.origin.rpy.0.into()
    }

    /// Lower limit of the joint.
    #[getter]
    pub fn limit_lower(&self) -> f64 {
        self.0.limit.lower
    }

    /// Upper limit of the joint.
    #[getter]
    pub fn limit_upper(&self) -> f64 {
        self.0.limit.upper
    }

    /// Effort limit of the joint.
    #[getter]
    pub fn limit_effort(&self) -> f64 {
        self.0.limit.effort
    }

    /// Velocity limit of the joint.
    #[getter]
    pub fn limit_velocity(&self) -> f64 {
        self.0.limit.velocity
    }

    /// Compute the transform components for this joint at the given value.
    ///
    /// The result is wrapped in a dictionary for easy conversion to the final types in Python.
    ///
    /// If `clamp` is true, values outside joint limits will be clamped and a warning is generated.
    /// If `clamp` is false (default), values outside limits are used as-is without warnings.
    #[pyo3(signature = (value, clamp = false))]
    pub fn compute_transform(&self, py: Python<'_>, value: f64, clamp: bool) -> PyResult<PyObject> {
        match urdf_joint_transform::internal::compute_joint_transform(&self.0, value, clamp) {
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
                dict.set_item("parent_frame", result.parent_frame)?;
                dict.set_item("child_frame", result.child_frame)?;
                dict.set_item("warning", result.warning)?;

                Ok(dict.into())
            }
            Err(e @ urdf_joint_transform::Error::UnsupportedJointType(_)) => {
                Err(PyNotImplementedError::new_err(e.to_string()))
            }
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "UrdfJoint(name={:?}, type={}, parent={:?}, child={:?})",
            self.0.name,
            &self.joint_type(),
            self.0.parent.link,
            self.0.child.link
        )
    }
}

/// URDF link
#[pyclass(name = "_UrdfLinkInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq], non-trivial implementation
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
