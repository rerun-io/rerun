use std::{
    path::Path,
    sync::{Arc, mpsc::Sender},
};

use ahash::{HashMap, HashMapExt as _, HashSet, HashSetExt as _};
use anyhow::Context as _;
use urdf_rs::{Geometry, Joint, Link, Robot};

use re_chunk::{ChunkBuilder, ChunkId, EntityPath, RowId, TimePoint};
use re_log_types::StoreId;
use re_types::{
    ComponentDescriptor, SerializedComponentBatch,
    archetypes::{Asset3D, Transform3D},
};

use crate::{DataLoader, DataLoaderError, LoadedData};

fn is_urdf_file(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("urdf"))
}

/// A [`DataLoader`] for `LeRobot` datasets.
///
/// An example dataset which can be loaded can be found on Hugging Face: [lerobot/pusht_image](https://huggingface.co/datasets/lerobot/pusht_image)
pub struct UrdfDataLoader;

impl DataLoader for UrdfDataLoader {
    fn name(&self) -> crate::DataLoaderName {
        "URDF Loader".to_owned()
    }

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

        load_urdf_file(&filepath, &tx, &settings.store_id)
            .with_context(|| "Failed to load URDF file!")?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        _filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: Sender<LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        todo!()
    }
}

struct UrdfTree {
    root: Link,
    links: HashMap<String, Link>,
    children: HashMap<String, Vec<Joint>>,
}

impl UrdfTree {
    fn new(robot: Robot) -> anyhow::Result<Self> {
        let mut links = HashMap::<String, Link>::new();
        let mut children = HashMap::<String, Vec<Joint>>::new();
        let mut child_links = HashSet::<String>::new();

        for link in robot.links {
            links.insert(link.name.clone(), link);
        }
        for joint in robot.joints {
            children
                .entry(joint.parent.link.clone())
                .or_default()
                .push(joint.clone());

            child_links.insert(joint.child.link.clone());
        }

        let root = links
            .iter()
            .find_map(|(name, link)| {
                if !child_links.contains(name) {
                    Some(link)
                } else {
                    None
                }
            })
            .with_context(|| "No root link found in URDF")?;

        Ok(Self {
            root: root.clone(),
            links,
            children,
        })
    }
}

fn load_urdf_file(
    filepath: &Path,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
) -> anyhow::Result<()> {
    let robot =
        urdf_rs::read_file(filepath).with_context(|| format!("Path: {}", filepath.display()))?;

    let root_dir = filepath
        .parent()
        .with_context(|| "Failed to get URDF parent directory")?;

    let urdf_tree = UrdfTree::new(robot).with_context(|| "Failed to build URDF tree!")?;
    let urdf_name = filepath
        .file_stem()
        .with_context(|| "Failed to get URDF file name")?;

    walk_tree(
        &urdf_tree.root.name,
        &urdf_tree,
        &urdf_name.to_string_lossy().to_string().into(),
        tx,
        store_id,
        root_dir,
    )?;

    Ok(())
}

fn walk_tree(
    link_name: &str,
    urdf_tree: &UrdfTree,
    parent_path: &EntityPath,
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    root_dir: &Path,
) -> anyhow::Result<()> {
    let link = urdf_tree
        .links
        .get(link_name)
        .with_context(|| format!("Link {link_name:?} missing from map"))?;
    debug_assert_eq!(link_name, link.name);
    let link_path = parent_path.join(&link_name.into()); // TODO: ergonomics

    log_link(tx, store_id, root_dir, link, &link_path)?;

    let Some(joints) = urdf_tree.children.get(link_name) else {
        // if there's no more joints connecting this link to anything else we've reached the end of this branch.
        return Ok(());
    };

    for joint in joints {
        let joint_path = link_path.join(&joint.name.as_str().into());
        log_joint(tx, store_id, &joint_path, joint)?;

        walk_tree(
            &joint.child.link,
            urdf_tree,
            &joint_path,
            tx,
            store_id,
            root_dir,
        )?;
    }

    Ok(())
}

fn log_joint(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    joint_path: &EntityPath,
    joint: &Joint,
) -> anyhow::Result<()> {
    let Joint {
        name,
        joint_type,
        origin,
        parent: _,
        child: _,
        axis,
        limit,
        calibration,
        dynamics,
        mimic,
        safety_controller,
    } = joint;

    let transform = transform_from_pose(origin);

    let chunk = ChunkBuilder::new(ChunkId::new(), joint_path.clone())
        .with_archetype(RowId::new(), TimePoint::default(), &transform)
        .build()?;

    tx.send(LoadedData::Chunk(
        UrdfDataLoader.name(),
        store_id.clone(),
        chunk,
    ))?;

    log_debug_format(tx, store_id, joint_path.clone(), "joint_type", joint_type)?;
    log_debug_format(tx, store_id, joint_path.clone(), "axis", axis)?;
    log_debug_format(tx, store_id, joint_path.clone(), "limit", limit)?;
    if let Some(calibration) = calibration {
        log_debug_format(tx, store_id, joint_path.clone(), "calibration", calibration)?;
    }
    if let Some(dynamics) = dynamics {
        log_debug_format(tx, store_id, joint_path.clone(), "dynamics", dynamics)?;
    }
    if let Some(mimic) = mimic {
        log_debug_format(tx, store_id, joint_path.clone(), "mimic", mimic)?;
    }
    if let Some(safety_controller) = safety_controller {
        log_debug_format(
            tx,
            store_id,
            joint_path.clone(),
            "safety_controller",
            &safety_controller,
        )?;
    }

    Ok(())
}

fn transform_from_pose(origin: &urdf_rs::Pose) -> Transform3D {
    let translation = [
        origin.xyz[0] as f32,
        origin.xyz[1] as f32,
        origin.xyz[2] as f32,
    ];

    let quaternion = euler_to_quat_xyzw(
        origin.rpy[0] as f32,
        origin.rpy[1] as f32,
        origin.rpy[2] as f32,
    );

    Transform3D::from_translation(translation).with_quaternion(quaternion)
}

fn log_debug_format(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    entity_path: EntityPath,
    name: &str,
    value: &dyn std::fmt::Debug,
) -> anyhow::Result<()> {
    tx.send(LoadedData::Chunk(
        UrdfDataLoader.name(),
        store_id.clone(),
        ChunkBuilder::new(ChunkId::new(), entity_path)
            .with_serialized_batches(
                RowId::new(),
                TimePoint::default(),
                vec![SerializedComponentBatch {
                    descriptor: ComponentDescriptor::new(name),
                    array: Arc::new(arrow::array::StringArray::from(vec![format!("{value:#?}")])),
                }],
            )
            .build()?,
    ))?;
    Ok(())
}

fn log_link(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    root_dir: &Path,
    link: &urdf_rs::Link,
    link_entity: &EntityPath,
) -> anyhow::Result<()> {
    let urdf_rs::Link {
        name,
        inertial,
        visual,
        collision,
    } = link;

    log_debug_format(tx, store_id, link_entity.clone(), "inertial", &inertial)?;

    for (i, visual) in visual.iter().enumerate() {
        let urdf_rs::Visual {
            name,
            origin,
            geometry,
            material,
        } = visual;
        let name = name.clone().unwrap_or_else(|| format!("visual_{i}"));
        let vis_entity = link_entity.join(&name.into());
        log_debug_format(tx, store_id, vis_entity.clone(), "origin", &origin)?;
        log_geometry(
            tx,
            store_id,
            root_dir,
            vis_entity,
            geometry,
            material.as_ref(),
        )?;
    }

    for (i, collision) in collision.iter().enumerate() {
        let urdf_rs::Collision {
            name,
            origin,
            geometry,
        } = collision;
        let name = name.clone().unwrap_or_else(|| format!("collision_{i}"));
        let collision_entity = link_entity.join(&name.into());
        log_debug_format(tx, store_id, collision_entity.clone(), "origin", &origin)?;
        log_geometry(tx, store_id, root_dir, collision_entity, geometry, None)?;
    }

    Ok(())
}

fn log_geometry(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    root_dir: &Path,
    vis_entity: EntityPath,
    geometry: &Geometry,
    material: Option<&urdf_rs::Material>,
) -> Result<(), anyhow::Error> {
    match &geometry {
        Geometry::Mesh { filename, scale } => {
            if let Some(material) = material {
                let urdf_rs::Material {
                    name: _,
                    color,
                    texture,
                } = material;
                if color.is_some() {
                    re_log::warn_once!("Material color not supported");
                };
                if texture.is_some() {
                    re_log::warn_once!("Material texture not supported");
                }
            }

            let mesh_path = root_dir.join(filename);

            let asset3d = Asset3D::from_file_path(mesh_path.clone())
                .with_context(|| format!("failed to load asset from: {}", mesh_path.display()))?;

            if scale.is_some_and(|scale| scale != urdf_rs::Vec3([1.0; 3])) {
                re_log::warn_once!("Scaled meshes not supported");
            }

            tx.send(crate::LoadedData::Chunk(
                UrdfDataLoader.name(),
                store_id.clone(),
                ChunkBuilder::new(ChunkId::new(), vis_entity)
                    .with_archetype(RowId::new(), TimePoint::default(), &asset3d)
                    .build()?,
            ))?;
        }
        other => {
            re_log::warn_once!(
                "Unsupported geometry: {other:?}. Only meshes are currently supported."
            );
        }
    }
    Ok(())
}

fn euler_to_quat_xyzw(roll: f32, pitch: f32, yaw: f32) -> [f32; 4] {
    // TODO(emilk): we should be able to use glam for this
    let (hr, hp, hy) = (roll * 0.5, pitch * 0.5, yaw * 0.5);
    let (sr, cr) = (hr.sin(), hr.cos());
    let (sp, cp) = (hp.sin(), hp.cos());
    let (sy, cy) = (hy.sin(), hy.cos());

    let x = sr * cp * cy + cr * sp * sy;
    let y = cr * sp * cy - sr * cp * sy;
    let z = cr * cp * sy + sr * sp * cy;
    let w = cr * cp * cy - sr * sp * sy;

    let norm = (x * x + y * y + z * z + w * w).sqrt();
    if norm > 0.0 {
        [x / norm, y / norm, z / norm, w / norm]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}
