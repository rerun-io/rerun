use std::{path::Path, sync::mpsc::Sender};

use ahash::{HashMap, HashMapExt as _, HashSet, HashSetExt as _};
use anyhow::Context as _;
use re_chunk::{ChunkBuilder, ChunkId, RowId, TimePoint};
use re_log_types::StoreId;
use re_types::archetypes::{Asset3D, Transform3D};
use urdf_rs::{Geometry, Joint, Link, Robot};

use crate::{DataLoader, DataLoaderError};

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
        tx: Sender<crate::LoadedData>,
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
        _tx: Sender<crate::LoadedData>,
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
    tx: &Sender<crate::LoadedData>,
    store_id: &StoreId,
) -> Result<(), DataLoaderError> {
    let robot = urdf_rs::read_file(filepath).with_context(|| "Failed to parse URDF file!")?;

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
        &urdf_name.to_string_lossy(),
        tx,
        store_id,
        root_dir,
    )?;

    Ok(())
}

fn walk_tree(
    link_name: &str,
    urdf_tree: &UrdfTree,
    parent_path: &str,
    tx: &Sender<crate::LoadedData>,
    store_id: &StoreId,
    root_dir: &Path,
) -> anyhow::Result<()> {
    let link = urdf_tree
        .links
        .get(link_name)
        .with_context(|| "Link missing from map")?;
    let link_path = format!("{parent_path}/{link_name}");

    log_link_visuals(link_name, tx, store_id, root_dir, link, &link_path)?;

    let Some(joints) = urdf_tree.children.get(link_name) else {
        // if there's no more joints connecting this link to anything else we've reached the end of this branch.
        return Ok(());
    };

    for joint in joints {
        let joint_path = format!("{link_path}/{}", joint.name);
        log_joint_pose(tx, store_id, &joint_path, joint)?;

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

fn log_joint_pose(
    tx: &Sender<crate::LoadedData>,
    store_id: &StoreId,
    joint_path: &str,
    joint: &Joint,
) -> anyhow::Result<()> {
    let origin = &joint.origin;
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

    let chunk = ChunkBuilder::new(ChunkId::new(), joint_path.into())
        .with_archetype(
            RowId::new(),
            TimePoint::default(),
            &Transform3D::from_translation(translation).with_quaternion(quaternion),
        )
        .build()?;

    tx.send(crate::LoadedData::Chunk(
        UrdfDataLoader.name(),
        store_id.clone(),
        chunk,
    ))?;

    Ok(())
}

fn log_link_visuals(
    link_name: &str,
    tx: &Sender<crate::LoadedData>,
    store_id: &StoreId,
    root_dir: &Path,
    link: &urdf_rs::Link,
    link_path: &String,
) -> anyhow::Result<()> {
    for (i, vis) in link.visual.iter().enumerate() {
        match &vis.geometry {
            Geometry::Mesh { filename, scale: _ } => {
                let mesh_path = root_dir.join(filename);

                let asset3d = Asset3D::from_file_path(mesh_path.clone()).with_context(|| {
                    format!("failed to load asset from: {}", mesh_path.display())
                })?;
                let chunk =
                    ChunkBuilder::new(ChunkId::new(), format!("{link_path}/visual_{i}").into())
                        .with_archetype(RowId::new(), TimePoint::default(), &asset3d)
                        .build()?;

                tx.send(crate::LoadedData::Chunk(
                    UrdfDataLoader.name(),
                    store_id.clone(),
                    chunk,
                ))?;
            }
            other => {
                anyhow::bail!(
                    "Link '{}' has unsupported geometry: {:?}. Only meshes are allowed.",
                    link_name,
                    other
                );
            }
        }
    }

    Ok(())
}

fn euler_to_quat_xyzw(roll: f32, pitch: f32, yaw: f32) -> [f32; 4] {
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
