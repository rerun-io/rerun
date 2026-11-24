use std::path::{Path, PathBuf};

use ahash::{HashMap, HashMapExt as _, HashSet, HashSetExt as _};
use anyhow::{Context as _, bail};
use arrow::array::ListArray;
use itertools::Itertools as _;
use re_chunk::{Chunk, EntityPath, TimePoint};
use re_log_types::EntityPathPart;
use re_sdk_types::archetypes::Transform3D;
use urdf_rs::{Geometry, Joint, JointType, Link, Material, Robot};

use super::joint_transform;
use crate::kinematic_tree::LogPaths;
use crate::{JointTransform, KinematicTree, ModelFormat, UnsupportedJointType};

/// Returns a short name for the geometry type, used as a path segment.
fn geometry_type_name(geometry: &Geometry) -> &'static str {
    match geometry {
        Geometry::Mesh { .. } => "mesh",
        Geometry::Box { .. } => "box",
        Geometry::Cylinder { .. } => "cylinder",
        Geometry::Capsule { .. } => "capsule",
        Geometry::Sphere { .. } => "sphere",
    }
}

/// A `.urdf` file loaded into memory (excluding any mesh files).
///
/// Can be used to inspect any link or joint in the URDF.
pub struct UrdfTree {
    /// The dir containing the .urdf file.
    ///
    /// Used to find mesh files (.stl etc) relative to the URDF file.
    pub(crate) urdf_dir: Option<PathBuf>,
    pub(crate) log_paths: LogPaths,

    name: String,
    root: Link,
    joints: Vec<Joint>,
    links: HashMap<String, Link>,
    children: HashMap<String, Vec<Joint>>,
    materials: HashMap<String, Material>,
    frame_prefix: Option<String>,
}

impl UrdfTree {
    /// Given a path to an `.urdf` file, load it.
    pub fn from_file_path<P: AsRef<Path>>(
        path: P,
        entity_path_prefix: Option<EntityPath>,
    ) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let robot = urdf_rs::read_file(path)?;
        let urdf_dir = path.parent().map(|p| p.to_path_buf());
        Self::new(robot, urdf_dir, entity_path_prefix)
    }

    /// The `urdf_dir` is the directory containing the `.urdf` file,
    /// which can later be used to resolve relative paths to mesh files.
    pub fn new(
        robot: Robot,
        urdf_dir: Option<PathBuf>,
        entity_path_prefix: Option<EntityPath>,
    ) -> anyhow::Result<Self> {
        let urdf_rs::Robot {
            name,
            version: _,
            links,
            joints,
            materials,
        } = robot;

        let materials = materials
            .into_iter()
            .map(|material| (material.name.clone(), material))
            .collect::<HashMap<_, _>>();

        let links: HashMap<String, Link> = links
            .into_iter()
            .map(|link| (link.name.clone(), link))
            .collect();

        let mut children = HashMap::<String, Vec<Joint>>::new();
        let mut child_links = HashSet::<String>::new();

        for joint in &joints {
            children
                .entry(joint.parent.link.clone())
                .or_default()
                .push(joint.clone());

            child_links.insert(joint.child.link.clone());
        }

        let roots = links
            .iter()
            .filter(|(name, _)| !child_links.contains(*name))
            .map(|(_, link)| link)
            .collect_vec();

        let root = match roots.len() {
            0 => {
                bail!("No root link found in URDF");
            }
            1 => roots[0].clone(),
            _ => {
                bail!("Multiple roots in URDF");
            }
        };

        for joint in &joints {
            if !links.contains_key(&joint.child.link) {
                bail!(
                    "Joint '{}' references unknown child link '{}'",
                    joint.name,
                    joint.child.link
                );
            }
            if !links.contains_key(&joint.parent.link) {
                bail!(
                    "Joint '{}' references unknown parent link '{}'",
                    joint.name,
                    joint.parent.link
                );
            }
            let moves_along_axis = matches!(
                joint.joint_type,
                JointType::Continuous | JointType::Revolute | JointType::Prismatic
            );
            if moves_along_axis && super::unit_joint_axis(joint).is_none() {
                let joint_name = &joint.name;
                re_log::warn!(
                    "Joint {joint_name:?} has a zero-length or non-finite axis, so it will not move. Set a non-zero `<axis xyz>` in the URDF."
                );
            }
        }

        let log_paths = LogPaths::new(&name, entity_path_prefix);

        Ok(Self {
            urdf_dir,
            name,
            root: root.clone(),
            joints,
            links,
            children,
            materials,
            log_paths,
            frame_prefix: None,
        })
    }

    /// Set the frame prefix applied to all frame IDs.
    pub fn with_frame_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.frame_prefix = Some(prefix.into());
        self
    }

    /// Set the entity path used for static transforms emitted by this URDF tree.
    ///
    /// This path is not affected by the tree's entity path prefix.
    pub fn with_static_transform_entity(mut self, entity_path: impl Into<EntityPath>) -> Self {
        self.log_paths.transforms = entity_path.into();
        self
    }

    /// Name of the robot defined in the URDF.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Computes a [`Transform3D`] for a joint at the given value.
    ///
    /// `value` is an angle in radians for revolute and continuous joints, and a distance in meters for
    /// prismatic joints.
    /// If [`KinematicTree::frame_prefix`] is set, the frame IDs of the transform are prefixed with it.
    /// If `clamp` is true, values outside the joint's limits are clamped and a warning is logged.
    pub fn compute_joint_transform(
        &self,
        joint: &Joint,
        value: f64,
        clamp: bool,
    ) -> Result<Transform3D, UnsupportedJointType> {
        let transform = self.prefixed_joint_transform(joint, value, clamp)?;
        for warning in &transform.warnings {
            re_log::warn!("{warning}");
        }
        Ok(transform.into())
    }

    fn prefixed_joint_transform(
        &self,
        joint: &Joint,
        value: f64,
        clamp: bool,
    ) -> Result<JointTransform, UnsupportedJointType> {
        Ok(
            joint_transform::compute_joint_transform(joint, value, clamp)?
                .with_frame_prefix(self.frame_prefix.as_deref()),
        )
    }

    /// See [`KinematicTree::compute_joint_transform_batches`].
    pub fn compute_joint_transform_batches(
        &self,
        names: &ListArray,
        values: &ListArray,
        clamp: bool,
    ) -> anyhow::Result<ListArray> {
        KinematicTree::compute_joint_transform_batches(self, names, values, clamp)
    }

    /// The root [`Link`] in the URDF hierarchy.
    pub fn root(&self) -> &Link {
        &self.root
    }

    pub fn joints(&self) -> impl Iterator<Item = &Joint> {
        self.joints.iter()
    }

    pub fn get_joint_by_name(&self, joint_name: &str) -> Option<&Joint> {
        self.joints.iter().find(|j| j.name == joint_name)
    }

    /// Returns the [`Link`] with the given `name`, if it exists.
    pub fn get_link(&self, link_name: &str) -> Option<&Link> {
        self.links.get(link_name)
    }

    /// Returns the child [`Joint`]s of the link with the given `name`, if any.
    pub fn get_children(&self, link_name: &str) -> Option<&Vec<Joint>> {
        self.children.get(link_name)
    }

    /// Get the visual geometries of a link and their entity paths, if any.
    pub fn get_visual_geometries(
        &self,
        link: &Link,
    ) -> Option<Vec<(EntityPath, &urdf_rs::Visual)>> {
        let link = self.links.get(&link.name)?;
        if link.visual.is_empty() {
            return None;
        }

        // The base path for all visual geometries of this link.
        // We use flat paths under `visual_root` since link names have to be unique and to avoid deep nesting.
        let visual_base_path_for_link =
            self.log_paths.visual_root.clone() / EntityPathPart::new(&link.name);

        // Collect all the link's visual geometries and build their entity paths.
        link.visual
            .iter()
            .enumerate()
            .map(|(i, visual)| {
                let visual_name = visual.name.clone().unwrap_or_else(|| format!("visual_{i}"));
                (
                    visual_base_path_for_link.clone() / EntityPathPart::new(visual_name),
                    visual,
                )
            })
            .collect::<Vec<_>>()
            .into()
    }

    /// Get the collision geometries of a link and their entity paths, if any.
    ///
    /// Collision geometries are organized by geometry type under `collision_root`:
    /// `collision_geometries/{geometry_type}/{link_name}/{collision_name}`.
    /// This makes it easy to toggle visibility per geometry type (e.g. hide meshes but keep primitives).
    pub fn get_collision_geometries(
        &self,
        link: &Link,
    ) -> Option<Vec<(EntityPath, &urdf_rs::Collision)>> {
        let link = self.links.get(&link.name)?;
        if link.collision.is_empty() {
            return None;
        }

        // Collect all the link's collision geometries and build their entity paths,
        // organized by geometry type for easy per-type visibility toggling.
        link.collision
            .iter()
            .enumerate()
            .map(|(i, collision)| {
                let geometry_type = geometry_type_name(&collision.geometry);
                let collision_name = collision
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("collision_{i}"));
                let path = self.log_paths.collision_root.clone()
                    / EntityPathPart::new(geometry_type)
                    / EntityPathPart::new(&link.name)
                    / EntityPathPart::new(collision_name);
                (path, collision)
            })
            .collect::<Vec<_>>()
            .into()
    }

    pub fn get_joint_child(&self, joint: &Joint) -> &Link {
        &self.links[&joint.child.link] // Safe because we checked that the joint's child link exists in `new()`
    }

    /// Get a material by name, if it exists.
    pub(crate) fn get_material(&self, name: &str) -> Option<&Material> {
        self.materials.get(name)
    }
}

impl KinematicTree for UrdfTree {
    fn model_format(&self) -> ModelFormat {
        ModelFormat::Urdf
    }

    fn frame_prefix(&self) -> Option<&str> {
        self.frame_prefix.as_deref()
    }

    fn emit(
        &self,
        emit: &mut dyn FnMut(Chunk),
        timepoint: &TimePoint,
        include_joint_transforms: bool,
    ) -> anyhow::Result<()> {
        Self::emit(self, emit, timepoint, include_joint_transforms)
    }

    fn compute_joint_transforms(
        &self,
        joint_values: &[(&str, f64)],
        clamp: bool,
    ) -> anyhow::Result<Vec<JointTransform>> {
        joint_values
            .iter()
            .map(|&(joint_name, value)| {
                let joint = self
                    .get_joint_by_name(joint_name)
                    .with_context(|| format!("URDF does not contain joint {joint_name:?}"))?;
                Ok(self.prefixed_joint_transform(joint, value, clamp)?)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_link(name: &str) -> urdf_rs::Link {
        urdf_rs::Link {
            name: name.to_owned(),
            inertial: Default::default(),
            visual: vec![],
            collision: vec![],
        }
    }

    /// Two robots that share the default `/tf_static` entity keep all of their static joint
    /// transforms, because the store keeps every static named-transform row.
    #[test]
    fn test_two_prefixed_robots_keep_their_static_transforms() {
        const URDF: &str = r#"
            <robot name="arm">
              <link name="base"/><link name="upper"/><link name="lower"/>
              <joint name="shoulder" type="fixed"><parent link="base"/><child link="upper"/></joint>
              <joint name="elbow" type="fixed"><parent link="upper"/><child link="lower"/></joint>
            </robot>"#;

        let mut store = re_chunk_store::ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test"),
            Default::default(),
        );
        let mut transforms_paths = Vec::new();
        for side in ["left", "right"] {
            let robot = urdf_rs::read_from_string(URDF).expect("the test URDF should parse");
            let tree = UrdfTree::new(robot, None, Some(EntityPath::from(side)))
                .expect("the test URDF should load")
                .with_frame_prefix(format!("{side}/"));
            transforms_paths.push(tree.log_paths.transforms.clone());
            tree.emit(
                &mut |chunk| {
                    store
                        .insert_chunk(&std::sync::Arc::new(chunk))
                        .expect("insert should succeed");
                },
                &TimePoint::STATIC,
                true,
            )
            .expect("emit should succeed");
        }

        assert_eq!(
            transforms_paths[0], transforms_paths[1],
            "both robots should share the default static transform entity"
        );

        let mut child_frames = store
            .iter_physical_chunks()
            .filter(|chunk| chunk.entity_path() == &transforms_paths[0])
            .flat_map(|chunk| {
                chunk
                    .iter_slices::<String>(Transform3D::descriptor_child_frame().component)
                    .flatten()
                    .map(|frame| frame.to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        child_frames.sort();
        assert_eq!(
            child_frames,
            ["left/lower", "left/upper", "right/lower", "right/upper"]
        );
    }

    /// A zero axis is a broken joint, not a broken robot: the model still loads, and the joint stays still.
    #[test]
    fn test_zero_axis_joint_does_not_move() {
        const URDF: &str = r#"
            <robot name="arm">
              <link name="a"/><link name="b"/>
              <joint name="broken_hinge" type="revolute">
                <parent link="a"/><child link="b"/><axis xyz="0 0 0"/>
                <limit lower="-1" upper="1" effort="1" velocity="1"/>
              </joint>
            </robot>"#;

        let robot = urdf_rs::read_from_string(URDF).expect("the test URDF should parse");
        let tree = UrdfTree::new(robot, None, None).expect("a zero axis must not fail the load");
        let joint = tree
            .get_joint_by_name("broken_hinge")
            .expect("the test URDF declares this joint");
        let transform = joint_transform::compute_joint_transform(joint, 0.5, false)
            .expect("a revolute joint is supported");
        assert_eq!(
            transform.quaternion,
            re_sdk_types::external::glam::Quat::IDENTITY
        );
    }

    #[test]
    fn test_apply_frame_prefix_without_prefix() {
        let robot = urdf_rs::Robot {
            name: "test".to_owned(),
            version: None,
            links: vec![make_minimal_link("base")],
            joints: vec![],
            materials: vec![],
        };
        let tree = UrdfTree::new(robot, None, None).unwrap();
        assert_eq!(tree.apply_frame_prefix("base"), "base");
        assert!(tree.frame_prefix().is_none());
    }

    #[test]
    fn test_apply_frame_prefix_with_prefix() {
        let robot = urdf_rs::Robot {
            name: "test".to_owned(),
            version: None,
            links: vec![make_minimal_link("base")],
            joints: vec![],
            materials: vec![],
        };
        let tree = UrdfTree::new(robot, None, None)
            .unwrap()
            .with_frame_prefix("left_arm/");
        assert_eq!(tree.apply_frame_prefix("base"), "left_arm/base");
        assert_eq!(tree.frame_prefix(), Some("left_arm/"));
    }

    #[test]
    fn test_with_static_transform_entity_overrides_default_path() {
        let robot = urdf_rs::Robot {
            name: "test".to_owned(),
            version: None,
            links: vec![make_minimal_link("base")],
            joints: vec![],
            materials: vec![],
        };
        let tree = UrdfTree::new(robot, None, None)
            .unwrap()
            .with_static_transform_entity("robot/tf");

        assert_eq!(tree.log_paths.transforms.to_string(), "/robot/tf");
    }
}
