use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;

use ahash::{HashMap, HashMapExt as _, HashSet, HashSetExt as _};
use anyhow::{Context as _, bail};
use itertools::Itertools as _;
use re_chunk::{ChunkBuilder, ChunkId, EntityPath, RowId, TimePoint};
use re_log_types::{EntityPathPart, StoreId};
use re_sdk_types::archetypes::{Asset3D, CoordinateFrame, InstancePoses3D, Transform3D};
use re_sdk_types::datatypes::Vec3D;
use re_sdk_types::external::glam;
use re_sdk_types::{AsComponents, Component as _, ComponentDescriptor, SerializedComponentBatch};
use urdf_rs::{Geometry, Joint, Link, Material, Robot, Vec3, Vec4};

use crate::{DataLoader, DataLoaderError, LoadedData};

/// Helper function to apply transform frame prefix to a frame ID.
fn apply_frame_prefix(frame_id: String, prefix: &Option<String>) -> String {
    match prefix {
        Some(prefix) => format!("{prefix}{frame_id}"),
        None => frame_id,
    }
}

fn is_urdf_file(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("urdf"))
}

fn send_chunk_builder(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    chunk: ChunkBuilder,
) -> anyhow::Result<()> {
    tx.send(LoadedData::Chunk(
        UrdfDataLoader.name(),
        store_id.clone(),
        chunk.build()?,
    ))?;
    Ok(())
}

fn send_archetype(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    timepoint: &TimePoint,
    archetype: &impl AsComponents,
) -> anyhow::Result<()> {
    send_chunk_builder(
        tx,
        store_id,
        ChunkBuilder::new(ChunkId::new(), entity_path).with_archetype(
            RowId::new(),
            timepoint.clone(),
            archetype,
        ),
    )
}

/// A [`DataLoader`] for [URDF](https://en.wikipedia.org/wiki/URDF) (Unified Robot Description Format),
/// common in ROS.
pub struct UrdfDataLoader;

impl DataLoader for UrdfDataLoader {
    fn name(&self) -> crate::DataLoaderName {
        "URDF Loader".to_owned()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        if !is_urdf_file(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let robot = urdf_rs::read_file(&filepath)
            .with_context(|| format!("Path: {}", filepath.display()))?;

        log_robot(
            robot,
            &filepath,
            &tx,
            &settings.opened_store_id_or_recommended(),
            &settings.entity_path_prefix,
            &settings.transform_frame_prefix,
            &settings.timepoint.clone().unwrap_or_default(),
        )
        .with_context(|| "Failed to load URDF file!")?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        if !is_urdf_file(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let robot = urdf_rs::read_from_string(&String::from_utf8_lossy(&contents))
            .with_context(|| format!("Path: {}", filepath.display()))?;

        log_robot(
            robot,
            &filepath,
            &tx,
            &settings.opened_store_id_or_recommended(),
            &settings.entity_path_prefix,
            &settings.transform_frame_prefix,
            &settings.timepoint.clone().unwrap_or_default(),
        )
        .with_context(|| "Failed to load URDF file!")?;

        Ok(())
    }
}

/// A `.urdf` file loaded into memory (excluding any mesh files).
///
/// Can be used to find the [`EntityPath`] of any link or joint in the URDF.
pub struct UrdfTree {
    /// The dir containing the .urdf file.
    ///
    /// Used to find mesh files (.stl etc) relative to the URDF file.
    urdf_dir: Option<PathBuf>,

    name: String,
    root: Link,
    joints: Vec<Joint>,
    links: HashMap<String, Link>,
    children: HashMap<String, Vec<Joint>>,
    materials: HashMap<String, Material>,
}

impl UrdfTree {
    /// Given a path to an `.urdf` file, load it.
    pub fn from_file_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let robot = urdf_rs::read_file(path)?;
        let urdf_dir = path.parent().map(|p| p.to_path_buf());
        Self::new(robot, urdf_dir)
    }

    /// The `urdf_dir` is the directory containing the `.urdf` file,
    /// which can later be used to resolve relative paths to mesh files.
    pub fn new(robot: Robot, urdf_dir: Option<PathBuf>) -> anyhow::Result<Self> {
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

        Ok(Self {
            urdf_dir,
            name,
            root: root.clone(),
            joints,
            links,
            children,
            materials,
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

    fn get_joint_path(&self, joint: &Joint) -> EntityPath {
        let parent_path = self.get_link_path_by_name(&joint.parent.link);
        parent_path / EntityPathPart::new(&joint.name)
    }

    fn get_link_path_by_name(&self, link_name: &str) -> EntityPath {
        if let Some(parent_joint) = self.get_parent_of_link(link_name) {
            self.get_joint_path(parent_joint) / EntityPathPart::new(link_name)
        } else {
            format!("{}/{link_name}", self.name).into()
        }
    }

    pub fn get_link_path(&self, link: &Link) -> EntityPath {
        self.get_link_path_by_name(&link.name)
    }

    /// Find the parent join of a link, if it exists.
    fn get_parent_of_link(&self, link_name: &str) -> Option<&Joint> {
        self.joints.iter().find(|j| j.child.link == link_name)
    }

    pub fn get_joint_child(&self, joint: &Joint) -> &Link {
        &self.links[&joint.child.link] // Safe because we checked that the joint's child link exists in `new()`
    }

    /// Get a material by name, if it exists.
    fn get_material(&self, name: &str) -> Option<&Material> {
        self.materials.get(name)
    }
}

fn log_robot(
    robot: urdf_rs::Robot,
    filepath: &Path,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path_prefix: &Option<EntityPath>,
    transform_frame_prefix: &Option<String>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    let urdf_dir = filepath.parent().map(|path| path.to_path_buf());

    let urdf_tree = UrdfTree::new(robot, urdf_dir).with_context(|| "Failed to build URDF tree!")?;
    let entity_path = entity_path_prefix
        .clone()
        .map(|prefix| prefix / EntityPath::from_single_string(urdf_tree.name.clone()))
        .unwrap_or_else(|| EntityPath::from_single_string(urdf_tree.name.clone()));

    // The robot's root coordinate frame_id.
    let root_frame = apply_frame_prefix(urdf_tree.root.name.clone(), transform_frame_prefix);
    send_archetype(
        tx,
        store_id,
        entity_path.clone(),
        timepoint,
        &CoordinateFrame::update_fields().with_frame(root_frame.clone()),
    )?;

    walk_tree(
        &urdf_tree,
        tx,
        store_id,
        &entity_path,
        &urdf_tree.root.name, // Note: has to be without prefix here!
        transform_frame_prefix,
        timepoint,
    )?;

    Ok(())
}

fn walk_tree(
    urdf_tree: &UrdfTree,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    parent_path: &EntityPath,
    link_name: &str,
    transform_frame_prefix: &Option<String>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    let link = urdf_tree
        .links
        .get(link_name)
        .with_context(|| format!("Link {link_name:?} missing from map"))?;
    debug_assert_eq!(link_name, link.name);
    let link_path = parent_path / EntityPathPart::new(link_name);

    log_link(
        urdf_tree,
        tx,
        store_id,
        link,
        &link_path,
        transform_frame_prefix,
        timepoint,
    )?;

    let Some(joints) = urdf_tree.children.get(link_name) else {
        // if there's no more joints connecting this link to anything else we've reached the end of this branch.
        return Ok(());
    };

    for joint in joints {
        let joint_path = &link_path / EntityPathPart::new(&joint.name);
        log_joint(
            tx,
            store_id,
            &joint_path,
            joint,
            transform_frame_prefix,
            timepoint,
        )?;

        // Recurse
        walk_tree(
            urdf_tree,
            tx,
            store_id,
            &joint_path,
            &joint.child.link, // Note: has to be without prefix here!
            transform_frame_prefix,
            timepoint,
        )?;
    }

    Ok(())
}

fn log_joint(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    joint_path: &EntityPath,
    joint: &Joint,
    transform_frame_prefix: &Option<String>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    let Joint {
        name: _,
        joint_type,
        origin,
        parent,
        child,
        axis,
        limit,
        calibration,
        dynamics,
        mimic,
        safety_controller,
    } = joint;

    // A joint's own coordinate frame is that of its parent link.
    let parent_frame = apply_frame_prefix(parent.link.clone(), transform_frame_prefix);
    send_archetype(
        tx,
        store_id,
        joint_path.clone(),
        timepoint,
        &CoordinateFrame::update_fields().with_frame(parent_frame.clone()),
    )?;
    // Send the joint origin, i.e. the default transform from parent link to child link.
    let child_frame = apply_frame_prefix(child.link.clone(), transform_frame_prefix);
    send_transform(
        tx,
        store_id,
        joint_path.clone(),
        origin,
        timepoint,
        parent_frame,
        child_frame,
    )?;

    log_debug_format(
        tx,
        store_id,
        joint_path.clone(),
        "joint_type",
        joint_type,
        timepoint,
    )?;
    log_debug_format(tx, store_id, joint_path.clone(), "axis", axis, timepoint)?;
    log_debug_format(tx, store_id, joint_path.clone(), "limit", limit, timepoint)?;
    if let Some(calibration) = calibration {
        log_debug_format(
            tx,
            store_id,
            joint_path.clone(),
            "calibration",
            calibration,
            timepoint,
        )?;
    }
    if let Some(dynamics) = dynamics {
        log_debug_format(
            tx,
            store_id,
            joint_path.clone(),
            "dynamics",
            dynamics,
            timepoint,
        )?;
    }
    if let Some(mimic) = mimic {
        log_debug_format(tx, store_id, joint_path.clone(), "mimic", mimic, timepoint)?;
    }
    if let Some(safety_controller) = safety_controller {
        log_debug_format(
            tx,
            store_id,
            joint_path.clone(),
            "safety_controller",
            &safety_controller,
            timepoint,
        )?;
    }

    Ok(())
}

fn transform_from_pose(
    origin: &urdf_rs::Pose,
    parent_frame: String,
    child_frame: String,
) -> Transform3D {
    let urdf_rs::Pose { xyz, rpy } = origin;
    let translation = [xyz[0] as f32, xyz[1] as f32, xyz[2] as f32];
    let quaternion = quat_xyzw_from_roll_pitch_yaw(rpy[0] as f32, rpy[1] as f32, rpy[2] as f32);
    Transform3D::update_fields()
        .with_translation(translation)
        .with_quaternion(quaternion)
        .with_parent_frame(parent_frame)
        .with_child_frame(child_frame)
}

fn instance_poses_from_pose(origin: &urdf_rs::Pose) -> InstancePoses3D {
    let urdf_rs::Pose { xyz, rpy } = origin;
    let translation = Vec3D::new(xyz[0] as f32, xyz[1] as f32, xyz[2] as f32);
    let quaternion = quat_xyzw_from_roll_pitch_yaw(rpy[0] as f32, rpy[1] as f32, rpy[2] as f32);
    InstancePoses3D::update_fields()
        .with_translations(vec![translation])
        .with_quaternions(vec![quaternion])
}

fn send_transform(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    origin: &urdf_rs::Pose,
    timepoint: &TimePoint,
    parent_frame: String,
    child_frame: String,
) -> anyhow::Result<()> {
    send_archetype(
        tx,
        store_id,
        entity_path,
        timepoint,
        &transform_from_pose(origin, parent_frame, child_frame),
    )
}

fn send_instance_pose_with_frame(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    timepoint: &TimePoint,
    origin: &urdf_rs::Pose,
    parent_frame: String,
) -> anyhow::Result<()> {
    send_archetype(
        tx,
        store_id,
        entity_path.clone(),
        timepoint,
        &instance_poses_from_pose(origin),
    )?;
    send_archetype(
        tx,
        store_id,
        entity_path,
        timepoint,
        &CoordinateFrame::update_fields().with_frame(parent_frame),
    )
}

/// Log the given value using its `Debug` formatting.
///
/// TODO(#402): support dynamic structured logging
fn log_debug_format(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    name: &str,
    value: &dyn std::fmt::Debug,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    send_chunk_builder(
        tx,
        store_id,
        ChunkBuilder::new(ChunkId::new(), entity_path).with_serialized_batches(
            RowId::new(),
            timepoint.clone(),
            vec![SerializedComponentBatch {
                descriptor: ComponentDescriptor::partial(name),
                array: Arc::new(arrow::array::StringArray::from(vec![format!("{value:#?}")])),
            }],
        ),
    )
}

fn log_link(
    urdf_tree: &UrdfTree,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    link: &urdf_rs::Link,
    link_entity: &EntityPath,
    transform_frame_prefix: &Option<String>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    let urdf_rs::Link {
        name: _,
        inertial,
        visual,
        collision,
    } = link;

    log_debug_format(
        tx,
        store_id,
        link_entity.clone(),
        "inertial",
        &inertial,
        timepoint,
    )?;

    // Log coordinate frame ID of the link.
    let link_name = apply_frame_prefix(link.name.clone(), transform_frame_prefix);
    send_archetype(
        tx,
        store_id,
        link_entity.clone(),
        timepoint,
        &CoordinateFrame::update_fields().with_frame(link_name.clone()),
    )?;

    for (i, visual) in visual.iter().enumerate() {
        let urdf_rs::Visual {
            name,
            origin,
            geometry,
            material,
        } = visual;
        let visual_name = name.clone().unwrap_or_else(|| format!("visual_{i}"));
        let visual_entity = link_entity / EntityPathPart::new(visual_name.clone());

        // Prefer inline defined material properties if present, otherwise fall back to global material.
        let material = material.as_ref().and_then(|mat| {
            if mat.color.is_some() || mat.texture.is_some() {
                Some(mat)
            } else {
                urdf_tree.get_material(&mat.name)
            }
        });

        // A visual geometry has no frame ID of its own and has a constant pose,
        // so we attach it to the link using an instance pose.
        send_instance_pose_with_frame(
            tx,
            store_id,
            visual_entity.clone(),
            timepoint,
            origin,
            link_name.clone(),
        )?;

        log_geometry(
            urdf_tree,
            tx,
            store_id,
            visual_entity,
            geometry,
            material,
            timepoint,
        )?;
    }

    for (i, collision) in collision.iter().enumerate() {
        let urdf_rs::Collision {
            name,
            origin,
            geometry,
        } = collision;
        let collision_name = name.clone().unwrap_or_else(|| format!("collision_{i}"));
        let collision_entity = link_entity / EntityPathPart::new(collision_name.clone());

        // A collision geometry has no frame ID of its own and has a constant pose,
        // so we attach it to the link using an instance pose.
        send_instance_pose_with_frame(
            tx,
            store_id,
            collision_entity.clone(),
            timepoint,
            origin,
            link_name.clone(),
        )?;

        log_geometry(
            urdf_tree,
            tx,
            store_id,
            collision_entity.clone(),
            geometry,
            None,
            timepoint,
        )?;

        if false {
            // TODO(#6541): the viewer should respect the `Visible` component.
            send_chunk_builder(
                tx,
                store_id,
                ChunkBuilder::new(ChunkId::new(), collision_entity).with_component_batch(
                    RowId::new(),
                    timepoint.clone(),
                    (
                        ComponentDescriptor {
                            archetype: None,
                            component: "visible".into(),
                            component_type: Some(re_sdk_types::components::Visible::name()),
                        },
                        &re_sdk_types::components::Visible::from(false),
                    ),
                ),
            )?;
        }
    }

    Ok(())
}

/// TODO(emilk): create a trait for this, so that one can use this URDF loader
/// from e.g. a ROS-bag loader.
#[cfg(target_arch = "wasm32")]
fn load_ros_resource(_root_dir: Option<&PathBuf>, resource_path: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::bail!("Loading ROS resources is not supported in WebAssembly: {resource_path}");
}

#[cfg(not(target_arch = "wasm32"))]
fn load_ros_resource(
    // Where the .urdf file is located.
    root_dir: Option<&PathBuf>,
    resource_path: &str,
) -> anyhow::Result<Vec<u8>> {
    if let Some((scheme, path)) = resource_path.split_once("://") {
        match scheme {
            "file" => std::fs::read(path).with_context(|| format!("Failed to read file: {path}")),
            "package" => read_ros_package_resource(root_dir, path),
            _ => {
                bail!("Unknown resource scheme: {scheme:?} in {resource_path}");
            }
        }
    } else {
        // Relative path
        if let Some(root_dir) = &root_dir {
            let full_path = root_dir.join(resource_path);
            std::fs::read(&full_path)
                .with_context(|| format!("Failed to read file: {}", full_path.display()))
        } else {
            bail!("No root directory set for URDF, cannot load resource: {resource_path}");
        }
    }
}

fn log_geometry(
    urdf_tree: &UrdfTree,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    geometry: &Geometry,
    material: Option<&urdf_rs::Material>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    match geometry {
        Geometry::Mesh { filename, scale } => {
            use re_sdk_types::components::MediaType;

            let mesh_bytes = load_ros_resource(urdf_tree.urdf_dir.as_ref(), filename)?;
            let mut asset3d =
                Asset3D::from_file_contents(mesh_bytes, MediaType::guess_from_path(filename));

            if let Some(material) = material {
                let urdf_rs::Material {
                    name: _,
                    color,
                    texture,
                } = material;
                if let Some(color) = color {
                    let urdf_rs::Color {
                        rgba: Vec4([r, g, b, a]),
                    } = color;
                    asset3d = asset3d.with_albedo_factor(
                        // TODO(emilk): is this linear or sRGB?
                        re_sdk_types::datatypes::Rgba32::from_linear_unmultiplied_rgba_f32(
                            *r as f32, *g as f32, *b as f32, *a as f32,
                        ),
                    );
                }
                if texture.is_some() {
                    re_log::warn_once!("Material texture not supported"); // TODO(emilk): support textures
                }
            }

            if let Some(scale) = scale
                && scale != &urdf_rs::Vec3([1.0; 3])
            {
                let urdf_rs::Vec3([x, y, z]) = *scale;
                send_archetype(
                    tx,
                    store_id,
                    entity_path.clone(),
                    timepoint,
                    &InstancePoses3D::update_fields().with_scales([(x as f32, y as f32, z as f32)]),
                )?;
            }

            send_archetype(tx, store_id, entity_path, timepoint, &asset3d)?;
        }
        Geometry::Box {
            size: Vec3([x, y, z]),
        } => {
            send_archetype(
                tx,
                store_id,
                entity_path,
                timepoint,
                &re_sdk_types::archetypes::Boxes3D::from_sizes([Vec3D::new(
                    *x as _, *y as _, *z as _,
                )]),
            )?;
        }
        Geometry::Cylinder { radius, length } => {
            // URDF and Rerun both use Z as the main axis
            send_archetype(
                tx,
                store_id,
                entity_path,
                timepoint,
                &re_sdk_types::archetypes::Cylinders3D::from_lengths_and_radii(
                    [*length as f32],
                    [*radius as f32],
                ),
            )?;
        }
        Geometry::Capsule { radius, length } => {
            // URDF and Rerun both use Z as the main axis
            send_archetype(
                tx,
                store_id,
                entity_path,
                timepoint,
                &re_sdk_types::archetypes::Capsules3D::from_lengths_and_radii(
                    [*length as f32],
                    [*radius as f32],
                ),
            )?;
        }
        Geometry::Sphere { radius } => {
            send_archetype(
                tx,
                store_id,
                entity_path,
                timepoint,
                &re_sdk_types::archetypes::Ellipsoids3D::from_radii([*radius as f32]),
            )?;
        }
    }
    Ok(())
}

fn quat_xyzw_from_roll_pitch_yaw(roll: f32, pitch: f32, yaw: f32) -> [f32; 4] {
    glam::Quat::from_euler(glam::EulerRot::ZYX, yaw, pitch, roll).to_array()
}

/// Read ROS package resource using the `package://` URI scheme.
///
/// This function resolves the package URI by looking up the package in the
/// `ROS_PACKAGE_PATH` (for ROS1) or `AMENT_PREFIX_PATH` (for ROS2), and reads the
/// resource from the resolved path.
/// If the path is relative, it will be resolved relative to the `root_dir` provided.
#[cfg(not(target_arch = "wasm32"))]
fn read_ros_package_resource(
    root_dir: Option<&PathBuf>,
    resource_path: &str,
) -> anyhow::Result<Vec<u8>> {
    let resolved_path = resolve_package_uri(resource_path)?;

    if resolved_path.is_absolute() {
        std::fs::read(&resolved_path).with_context(|| {
            format!(
                "Failed to read package resource: {}",
                resolved_path.display()
            )
        })
    } else if let Some(root_dir) = root_dir {
        // If the path is relative, resolve it relative to the `root_dir`.
        let full_path = root_dir.join(resolved_path);
        std::fs::read(&full_path)
            .with_context(|| format!("Failed to read file: {}", full_path.display()))
    } else {
        // If no `root_dir` is provided, we cannot resolve the relative path.
        bail!("No root directory set for URDF, cannot load resource: {resource_path}");
    }
}

/// Try to resolve the `pkg_name/rel/path` part of a ROS `package://` URI,
/// by scanning `ROS_PACKAGE_PATH` (ROS1) or `AMENT_PREFIX_PATH` (ROS2).
#[cfg(not(target_arch = "wasm32"))]
fn resolve_package_uri(uri: &str) -> anyhow::Result<PathBuf> {
    use std::env;

    let mut parts = uri.splitn(2, '/');
    let (pkg, rel) = parts
        .next()
        .and_then(|pkg| parts.next().map(|rel| (pkg, rel)))
        .ok_or_else(|| anyhow::anyhow!("Invalid package URI: {uri}"))?;

    let rel = PathBuf::from(rel);

    if rel.is_absolute() {
        // If the relative path is absolute, we can just return it.
        return Ok(rel);
    }

    // Try ROS1 and then ROS2 package paths, in case of a mixed environment.
    // ROS1: look in each entry of ROS_PACKAGE_PATH as `<entry>/pkg_name`
    if let Ok(val) = env::var("ROS_PACKAGE_PATH") {
        for root in env::split_paths(&val) {
            let candidate = root.join(pkg);
            if candidate.exists() {
                return Ok(candidate.join(rel));
            }
        }
    }

    // ROS2: look in each entry of AMENT_PREFIX_PATH as `<entry>/share/pkg_name`
    if let Ok(val) = env::var("AMENT_PREFIX_PATH") {
        for root in env::split_paths(&val) {
            let candidate = root.join("share").join(pkg);
            if candidate.exists() {
                return Ok(candidate.join(rel));
            }
        }
    }

    bail!(
        "Failed to resolve package URI: {uri}, tried `ROS_PACKAGE_PATH` and `AMENT_PREFIX_PATH`, but no matching package found"
    );
}
