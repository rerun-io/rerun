use std::path::{Path, PathBuf};

use ahash::{HashMap, HashMapExt as _, HashSet, HashSetExt as _};
use anyhow::bail;
use itertools::Itertools as _;
use re_chunk::EntityPath;
use re_log_types::EntityPathPart;
use urdf_rs::{Joint, Link, Material, Robot};

/// Helper struct containing the (root) entity paths where the different parts of the URDF model are logged.
pub(crate) struct UrdfLogPaths {
    /// The root entity path of the robot.
    pub root: EntityPath,

    /// We separate visual and collision geometries under different paths below `root_path`.
    /// This makes it for example easier to toggle the visibility of all visual or collision geometries at once.
    pub visual_root: EntityPath,
    pub collision_root: EntityPath,

    // We log all default transforms to the same entity path because we use Transform3D with frame names.
    pub transforms: EntityPath,
}

impl UrdfLogPaths {
    pub fn new(robot_name: &str, entity_path_prefix: Option<EntityPath>) -> Self {
        let root = entity_path_prefix
            .map(|prefix| prefix / EntityPath::from_single_string(robot_name))
            .unwrap_or_else(|| EntityPath::from_single_string(robot_name));
        let visual_root = root.clone() / EntityPathPart::new("visual_geometries");
        let collision_root = root.clone() / EntityPathPart::new("collision_geometries");
        let transforms = root.clone() / EntityPathPart::new("joint_transforms");

        Self {
            root,
            visual_root,
            collision_root,
            transforms,
        }
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
    pub(crate) log_paths: UrdfLogPaths,

    name: String,
    root: Link,
    joints: Vec<Joint>,
    links: HashMap<String, Link>,
    children: HashMap<String, Vec<Joint>>,
    materials: HashMap<String, Material>,
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
        }

        let log_paths = UrdfLogPaths::new(&name, entity_path_prefix);

        Ok(Self {
            urdf_dir,
            name,
            root: root.clone(),
            joints,
            links,
            children,
            materials,
            log_paths,
        })
    }

    /// Name of the robot defined in the URDF.
    pub fn name(&self) -> &str {
        &self.name
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
    pub fn get_collision_geometries(
        &self,
        link: &Link,
    ) -> Option<Vec<(EntityPath, &urdf_rs::Collision)>> {
        let link = self.links.get(&link.name)?;
        if link.collision.is_empty() {
            return None;
        }

        // The base path for all collision geometries of this link.
        // We use flat paths under `collision_root` since link names have to be unique and to avoid deep nesting.
        let collision_base_path_for_link =
            self.log_paths.collision_root.clone() / EntityPathPart::new(&link.name);

        // Collect all the link's collision geometries and build their entity paths.
        link.collision
            .iter()
            .enumerate()
            .map(|(i, collision)| {
                let collision_name = collision
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("collision_{i}"));
                (
                    collision_base_path_for_link.clone() / EntityPathPart::new(collision_name),
                    collision,
                )
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
