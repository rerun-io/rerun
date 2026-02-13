//! Rerun data loader and utilities for URDF files.

pub mod joint_transform;
mod urdf_tree;
pub use urdf_tree::UrdfTree;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use crossbeam::channel::Sender;
use re_chunk::{ChunkBuilder, ChunkId, EntityPath, RowId, TimePoint};
use re_log_types::StoreId;
use re_sdk_types::archetypes::{Asset3D, CoordinateFrame, InstancePoses3D, Transform3D};
use re_sdk_types::datatypes::Vec3D;
use re_sdk_types::external::glam;
use re_sdk_types::{AsComponents, Component as _, ComponentDescriptor};
use urdf_rs::{Geometry, Joint, Vec3, Vec4};

use crate::{DataLoader, DataLoaderError, LoadedData};

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
            &settings.timepoint.clone().unwrap_or_default(),
        )
        .with_context(|| "Failed to load URDF file!")?;

        Ok(())
    }
}

fn log_robot(
    robot: urdf_rs::Robot,
    filepath: &Path,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path_prefix: &Option<EntityPath>,
    timepoint: &TimePoint,
) -> anyhow::Result<()> {
    let urdf_dir = filepath.parent().map(|path| path.to_path_buf());

    let urdf_tree = UrdfTree::new(robot, urdf_dir, entity_path_prefix.clone())
        .with_context(|| "Failed to build URDF tree!")?;

    // The robot's root coordinate frame_id.
    send_archetype(
        tx,
        store_id,
        urdf_tree.log_paths.root.clone(),
        timepoint,
        &CoordinateFrame::update_fields().with_frame(urdf_tree.root().name.clone()),
    )?;

    let transforms = walk_tree(&urdf_tree, tx, store_id, timepoint, &urdf_tree.root().name)?;

    // Send all transforms as rows in a single chunk.
    if !transforms.is_empty() {
        send_static_transforms_batch(tx, store_id, &urdf_tree.log_paths.transforms, &transforms)?;
    }

    Ok(())
}

fn walk_tree(
    urdf_tree: &UrdfTree,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    timepoint: &TimePoint,
    link_name: &str,
) -> anyhow::Result<Vec<Transform3D>> {
    let link = urdf_tree
        .get_link(link_name)
        .with_context(|| format!("Link {link_name:?} missing from map"))?;
    re_log::debug_assert_eq!(link_name, link.name);

    log_link(urdf_tree, tx, store_id, timepoint, link)?;

    let Some(joints) = urdf_tree.get_children(link_name) else {
        // if there's no more joints connecting this link to anything else we've reached the end of this branch.
        return Ok(Vec::new());
    };

    let mut joint_transforms_for_link = Vec::new();
    for joint in joints {
        joint_transforms_for_link.push(get_joint_transform(joint));

        // Recurse
        let mut child_transforms =
            walk_tree(urdf_tree, tx, store_id, timepoint, &joint.child.link)?;
        joint_transforms_for_link.append(&mut child_transforms);
    }

    Ok(joint_transforms_for_link)
}

fn get_joint_transform(joint: &Joint) -> Transform3D {
    let Joint {
        name: _,
        joint_type: _,
        origin,
        parent,
        child,
        axis: _,
        limit: _,
        calibration: _,
        dynamics: _,
        mimic: _,
        safety_controller: _,
    } = joint;

    transform_from_pose(origin, parent.link.clone(), child.link.clone())
}

/// Send a batch of static transforms as a single chunk.
///
/// We always do this statically for URDF, because this allows users to override them later
/// on any other transform entity of their choice.
fn send_static_transforms_batch(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    transforms_path: &EntityPath,
    transforms: &[Transform3D],
) -> anyhow::Result<()> {
    let mut chunk = ChunkBuilder::new(ChunkId::new(), transforms_path.clone());

    for transform in transforms {
        chunk = chunk.with_archetype(RowId::new(), TimePoint::STATIC, transform);
    }

    send_chunk_builder(tx, store_id, chunk)
}

fn transform_from_pose(
    origin: &urdf_rs::Pose,
    parent_frame: String,
    child_frame: String,
) -> Transform3D {
    let urdf_rs::Pose { xyz, rpy } = origin;
    Transform3D::update_fields()
        .with_translation([xyz[0] as f32, xyz[1] as f32, xyz[2] as f32])
        .with_quaternion(quat_from_rpy(&rpy.0).to_array())
        .with_parent_frame(parent_frame)
        .with_child_frame(child_frame)
}

fn instance_poses_from_pose(origin: &urdf_rs::Pose, scale: Option<Vec3D>) -> InstancePoses3D {
    let urdf_rs::Pose { xyz, rpy } = origin;
    let mut poses = InstancePoses3D::update_fields()
        .with_translations(vec![Vec3D::new(
            xyz[0] as f32,
            xyz[1] as f32,
            xyz[2] as f32,
        )])
        .with_quaternions(vec![quat_from_rpy(&rpy.0).to_array()]);

    if let Some(scale) = scale {
        poses = poses.with_scales(vec![scale]);
    }

    poses
}

fn send_instance_pose_with_frame(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    timepoint: &TimePoint,
    origin: &urdf_rs::Pose,
    parent_frame: String,
    scale: Option<Vec3D>,
) -> anyhow::Result<()> {
    send_archetype(
        tx,
        store_id,
        entity_path.clone(),
        timepoint,
        &instance_poses_from_pose(origin, scale),
    )?;
    send_archetype(
        tx,
        store_id,
        entity_path,
        timepoint,
        &CoordinateFrame::update_fields().with_frame(parent_frame),
    )
}

fn extract_instance_scale(geometry: &Geometry) -> Option<Vec3D> {
    match geometry {
        Geometry::Mesh {
            scale: Some(Vec3([x, y, z])),
            ..
        } => Some(Vec3D::new(*x as f32, *y as f32, *z as f32)),
        _ => None,
    }
}

fn log_link(
    urdf_tree: &UrdfTree,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    timepoint: &TimePoint,
    link: &urdf_rs::Link,
) -> anyhow::Result<()> {
    let urdf_rs::Link {
        name: link_name,
        inertial: _,
        visual: _,
        collision: _,
    } = link;

    for (visual_entity_path, visual) in urdf_tree.get_visual_geometries(link).unwrap_or_default() {
        let urdf_rs::Visual {
            name: _,
            origin,
            geometry,
            material,
        } = visual;

        let instance_scale = extract_instance_scale(geometry);
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
            visual_entity_path.clone(),
            timepoint,
            origin,
            link_name.clone(),
            instance_scale,
        )?;

        log_geometry(
            urdf_tree,
            tx,
            store_id,
            visual_entity_path,
            geometry,
            material,
            timepoint,
        )?;
    }

    for (collision_entity_path, collision) in
        urdf_tree.get_collision_geometries(link).unwrap_or_default()
    {
        let urdf_rs::Collision {
            name: _,
            origin,
            geometry,
        } = collision;
        let instance_scale = extract_instance_scale(geometry);

        // A collision geometry has no frame ID of its own and has a constant pose,
        // so we attach it to the link using an instance pose.
        send_instance_pose_with_frame(
            tx,
            store_id,
            collision_entity_path.clone(),
            timepoint,
            origin,
            link_name.clone(),
            instance_scale,
        )?;

        log_geometry(
            urdf_tree,
            tx,
            store_id,
            collision_entity_path.clone(),
            geometry,
            None,
            timepoint,
        )?;

        if false {
            // TODO(michael): consider hiding collision geometries by default.
            // TODO(#6541): the viewer should respect the `Visible` component.
            send_chunk_builder(
                tx,
                store_id,
                ChunkBuilder::new(ChunkId::new(), collision_entity_path).with_component_batch(
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
    bail!("Loading ROS resources is not supported in WebAssembly: {resource_path}");
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
        Geometry::Mesh { filename, scale: _ } => {
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

fn quat_from_rpy(rpy: &[f64; 3]) -> glam::Quat {
    glam::Quat::from_euler(
        glam::EulerRot::ZYX,
        rpy[2] as f32,
        rpy[1] as f32,
        rpy[0] as f32,
    )
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
