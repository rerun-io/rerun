use std::collections::BTreeSet;
use std::path::Path;

use ahash::HashMap;
use anyhow::Context as _;
use mjcf_rs::Pose;
use mjcf_rs::body::{Geom, GeomType};
use re_chunk::{Chunk, EntityPath, TimePoint};
use re_log_types::EntityPathPart;
use re_sdk_types::archetypes::{CoordinateFrame, Transform3D};

use super::geometry::{MeshLoader, emit_geom};
use super::joint_transform;
use super::model::{
    self, MjcfBody, MjcfBodyId, MjcfJoint, MjcfJointId, MjcfModel, WORLD_BODY_NAME,
    model_name_from_path,
};
use crate::kinematic_tree::{LogPaths, emit_archetype, emit_static_transforms_batch};
use crate::{JointTransform, KinematicTree, ModelFormat};

/// Separates the body and joint name in the frame ID of an intermediate frame, see [`MjcfTree::joint_frames`].
const JOINT_FRAME_SEPARATOR: &str = "::";

/// Geoms in these groups are hidden by default in MuJoCo's viewer, and by convention hold collision geometry.
const FIRST_COLLISION_GROUP: i32 = 3;

/// The frames that a joint moves relative to each other, before applying the frame prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MjcfJointFrames {
    /// The parent body for a body's first joint, and otherwise the frame after the previous joint.
    pub parent: String,

    /// The body for a body's last joint, and otherwise the intermediate frame `{body}::{joint}`.
    pub child: String,
}

/// An [MJCF](https://mujoco.readthedocs.io/en/stable/XMLreference.html) model loaded into memory
/// (excluding any mesh files).
///
/// Every body has a frame named after it.
/// Its joints move it relative to its parent body, and a body with several joints has an intermediate
/// frame between each pair of them, see [`MjcfTree::joint_frames`].
pub struct MjcfTree {
    model: MjcfModel,
    log_paths: LogPaths,
    frame_prefix: Option<String>,
    body_ids: HashMap<String, MjcfBodyId>,
    joint_ids: HashMap<String, MjcfJointId>,
}

/// Visual geometries are grouped by body, and collision geometries first by type and then by body,
/// so that e.g. all collision meshes can be hidden at once.
fn geom_path(log_paths: &LogPaths, body_name: &str, geom: &Geom, geom_name: &str) -> EntityPath {
    if is_collision(geom) {
        log_paths.collision_root.clone()
            / EntityPathPart::new(model::geom_type_name(geom.type_))
            / EntityPathPart::new(body_name)
            / EntityPathPart::new(geom_name)
    } else {
        log_paths.visual_root.clone()
            / EntityPathPart::new(body_name)
            / EntityPathPart::new(geom_name)
    }
}

fn is_collision(geom: &Geom) -> bool {
    geom.group >= FIRST_COLLISION_GROUP
}

impl MjcfTree {
    /// Loads the `.mjcf` or `.xml` file at `path`, along with the files it includes.
    pub fn from_file_path(
        path: impl AsRef<Path>,
        entity_path_prefix: Option<EntityPath>,
    ) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|err| {
            anyhow::anyhow!(
                "Failed to read MJCF file: {err}\nFile path: {}",
                path.display()
            )
        })?;
        Self::from_xml(
            &text,
            path.parent().unwrap_or_else(|| Path::new("")),
            &model_name_from_path(path),
            entity_path_prefix,
        )
        .map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse MJCF file: {err:#}\nFile path: {}",
                path.display()
            )
        })
    }

    /// Parses an MJCF document.
    ///
    /// `model_dir` is the directory of the main model file, which `<include>`s and relative asset
    /// paths are resolved against.
    /// `fallback_name` is used if the model has no `model` attribute.
    pub fn from_xml(
        text: &str,
        model_dir: &Path,
        fallback_name: &str,
        entity_path_prefix: Option<EntityPath>,
    ) -> anyhow::Result<Self> {
        let model = MjcfModel::parse(text, model_dir, fallback_name)?;

        Ok(Self {
            log_paths: LogPaths::new(&model.name, entity_path_prefix),
            frame_prefix: None,
            body_ids: model
                .bodies
                .iter()
                .map(|body| (body.name.clone(), body.id))
                .collect(),
            joint_ids: model
                .joints
                .iter()
                .map(|joint| (joint.name.clone(), joint.id))
                .collect(),
            model,
        })
    }

    /// Set the frame prefix applied to all frame IDs.
    pub fn with_frame_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.frame_prefix = Some(prefix.into());
        self
    }

    /// Set the entity path used for static transforms emitted by this tree.
    ///
    /// This path is not affected by the tree's entity path prefix.
    pub fn with_static_transform_entity(mut self, entity_path: impl Into<EntityPath>) -> Self {
        self.log_paths.transforms = entity_path.into();
        self
    }

    /// Name of the model, from its `model` attribute or else its file name.
    pub fn name(&self) -> &str {
        &self.model.name
    }

    /// All bodies, parents before children, starting with the world body.
    pub fn bodies(&self) -> &[MjcfBody] {
        &self.model.bodies
    }

    /// The body with the given id, or `None` if there is none.
    pub fn get_body(&self, id: MjcfBodyId) -> Option<&MjcfBody> {
        self.model.body(id)
    }

    /// The body with the given name, or `None` if there is none.
    pub fn get_body_by_name(&self, body_name: &str) -> Option<&MjcfBody> {
        let &id = self.body_ids.get(body_name)?;
        self.model.body(id)
    }

    /// The parent of `body`, or `None` for the world body.
    pub fn get_parent_body(&self, body: &MjcfBody) -> Option<&MjcfBody> {
        self.model.body(body.parent?)
    }

    /// All joints, in the order of their bodies.
    pub fn joints(&self) -> &[MjcfJoint] {
        &self.model.joints
    }

    /// The joint with the given id, or `None` if there is none.
    pub fn get_joint(&self, id: MjcfJointId) -> Option<&MjcfJoint> {
        self.model.joint(id)
    }

    /// The joint with the given name, or `None` if there is none.
    pub fn get_joint_by_name(&self, joint_name: &str) -> Option<&MjcfJoint> {
        let &id = self.joint_ids.get(joint_name)?;
        self.model.joint(id)
    }

    /// The body that `joint` moves, or `None` if `joint` is not part of this model.
    pub fn get_joint_body(&self, joint: &MjcfJoint) -> Option<&MjcfBody> {
        if self.model.joint(joint.id) != Some(joint) {
            return None;
        }
        self.model.body(joint.body)
    }

    /// The entity paths of the visual geometries of `body`.
    pub fn get_visual_geometry_paths(&self, body: &MjcfBody) -> Vec<EntityPath> {
        self.geometry_paths(body, false)
    }

    /// The entity paths of the collision geometries of `body`.
    pub fn get_collision_geometry_paths(&self, body: &MjcfBody) -> Vec<EntityPath> {
        self.geometry_paths(body, true)
    }

    /// Empty if `body` is not part of this model.
    fn geometry_paths(&self, body: &MjcfBody, collision: bool) -> Vec<EntityPath> {
        if self.model.body(body.id) != Some(body) {
            return Vec::new();
        }
        let Some(source_body) = self.model.source_body(body.id) else {
            return Vec::new();
        };
        model::drawable_geoms(source_body)
            .filter(|(_, geom)| is_collision(geom) == collision)
            .map(|(geom_name, geom)| geom_path(&self.log_paths, &body.name, geom, &geom_name))
            .collect()
    }

    /// Computes the transform of `joint` at `value`, see [`Self::joint_frames`] for its frames.
    ///
    /// `value` is an angle in radians for hinge joints, and a distance in meters for slide joints,
    /// in the same convention as MuJoCo's `qpos`.
    /// If [`Self::frame_prefix`] is set, the frame IDs of the transform are prefixed with it.
    /// If `clamp` is true, values outside a limited joint's range are clamped and a warning is added.
    /// Fails with a [`crate::UnsupportedJointType`] for ball and free joints, and if `joint` is not part
    /// of this model.
    pub fn compute_joint_transform(
        &self,
        joint: &MjcfJoint,
        value: f64,
        clamp: bool,
    ) -> anyhow::Result<JointTransform> {
        let not_in_model = || {
            format!(
                "Joint {:?} is not part of MJCF model {:?}",
                joint.name, self.model.name
            )
        };
        self.get_joint_body(joint).with_context(not_in_model)?;
        let (motion, warnings) = joint_transform::compute_joint_motion(joint, value, clamp)?;
        self.joint_frame_transform(joint, motion, warnings)
            .with_context(not_in_model)
    }

    /// The frames that `joint` moves relative to each other, or `None` if `joint` is not part of this model.
    ///
    /// A body with several joints has an intermediate frame `{body}::{joint}` after each joint but its
    /// last, so that every joint moves a frame of its own.
    /// The first joint's parent frame is the parent body, and the last joint's child frame is its body.
    pub fn joint_frames(&self, joint: &MjcfJoint) -> Option<MjcfJointFrames> {
        let body = self.get_joint_body(joint)?;
        let position = body.joints.iter().position(|&id| id == joint.id)?;
        let parent = if let Some(previous) = position.checked_sub(1) {
            self.frame_after_joint(body, previous)?
        } else {
            self.get_parent_body(body)
                .map_or(WORLD_BODY_NAME, |parent| parent.name.as_str())
                .to_owned()
        };
        Some(MjcfJointFrames {
            parent,
            child: self.frame_after_joint(body, position)?,
        })
    }

    /// The frame after the joint at `position` among the joints of `body`.
    fn frame_after_joint(&self, body: &MjcfBody, position: usize) -> Option<String> {
        let joint = self.model.joint(*body.joints.get(position)?)?;
        Some(if position + 1 == body.joints.len() {
            body.name.clone()
        } else {
            format!("{}{JOINT_FRAME_SEPARATOR}{}", body.name, joint.name)
        })
    }

    /// The transform of `joint` with `motion`, which also carries its body's reference pose if `joint` is the body's first.
    ///
    /// `None` if `joint` is not part of this model.
    fn joint_frame_transform(
        &self,
        joint: &MjcfJoint,
        motion: Pose,
        warnings: Vec<String>,
    ) -> Option<JointTransform> {
        let body = self.get_joint_body(joint)?;
        let pose = if body.joints.first() == Some(&joint.id) {
            body.pose * motion
        } else {
            motion
        };
        let MjcfJointFrames { parent, child } = self.joint_frames(joint)?;
        Some(JointTransform {
            translation: pose.translation.as_vec3(),
            quaternion: pose.rotation.as_quat(),
            parent_frame: self.apply_frame_prefix(&parent),
            child_frame: self.apply_frame_prefix(&child),
            warnings,
        })
    }

    /// Emit the full model (geometry + static body transforms) as [`Chunk`]s.
    pub fn emit(
        &self,
        emit: &mut dyn FnMut(Chunk),
        timepoint: &TimePoint,
        include_joint_transforms: bool,
    ) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let model = &self.model;

        emit_archetype(
            emit,
            self.log_paths.root.clone(),
            timepoint,
            &CoordinateFrame::update_fields().with_frame(self.apply_frame_prefix(WORLD_BODY_NAME)),
        )?;

        if include_joint_transforms && model.bodies.len() > 1 {
            let mut transforms = Vec::new();
            for body in &model.bodies {
                let Some(parent) = self.get_parent_body(body) else {
                    continue;
                };
                if body.joints.is_empty() {
                    transforms.push(Transform3D::from(JointTransform {
                        translation: body.pose.translation.as_vec3(),
                        quaternion: body.pose.rotation.as_quat(),
                        parent_frame: self.apply_frame_prefix(&parent.name),
                        child_frame: self.apply_frame_prefix(&body.name),
                        warnings: Vec::new(),
                    }));
                }
                for &joint_id in &body.joints {
                    let transform = model
                        .joint(joint_id)
                        .and_then(|joint| {
                            self.joint_frame_transform(joint, Pose::IDENTITY, Vec::new())
                        })
                        .with_context(|| {
                            format!(
                                "Body {:?} refers to a joint that is not part of the model",
                                body.name
                            )
                        })?;
                    transforms.push(Transform3D::from(transform));
                }
            }
            emit_static_transforms_batch(emit, &self.log_paths.transforms, &transforms)?;
        }

        let mut warnings = BTreeSet::new();
        let mut meshes = MeshLoader::new(&model.source, &model.model_dir);
        for (body, entry) in std::iter::zip(&model.bodies, &model.source.bodies) {
            let frame = self.apply_frame_prefix(&body.name);
            for geom in &entry.body.geoms {
                if matches!(geom.type_, GeomType::Hfield | GeomType::Sdf) {
                    warnings.insert(format!(
                        "Skipped geoms of unsupported type {:?}",
                        geom.type_
                    ));
                }
            }
            for (geom_name, geom) in model::drawable_geoms(&entry.body) {
                emit_geom(
                    emit,
                    geom_path(&self.log_paths, &body.name, geom, &geom_name),
                    &frame,
                    geom,
                    &mut meshes,
                    &mut warnings,
                    timepoint,
                )?;
            }
        }
        for warning in &warnings {
            re_log::warn!("MJCF model {:?}: {warning}", model.name);
        }
        meshes.log_failures(&model.name);

        Ok(())
    }
}

impl KinematicTree for MjcfTree {
    fn model_format(&self) -> ModelFormat {
        ModelFormat::Mjcf
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
                    .with_context(|| format!("MJCF model does not contain joint {joint_name:?}"))?;
                self.compute_joint_transform(joint, value, clamp)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use re_sdk_types::external::glam::{Quat, Vec3};

    use super::*;

    /// A body with two hinges (one off-center), a slide, and a body with a limited hinge and a ball joint.
    const MODEL: &str = r#"
        <mujoco model="arm">
          <compiler angle="radian"/>
          <worldbody>
            <body name="base">
              <body name="shoulder" pos="0 0 1">
                <joint name="yaw" type="hinge" axis="0 0 1"/>
                <joint name="pitch" type="hinge" axis="0 1 0" pos="1 0 0"/>
                <body name="slider" pos="1 0 0">
                  <joint name="extend" type="slide" axis="1 0 0" ref="0.5"/>
                </body>
                <body name="wrist">
                  <joint name="limited" axis="1 0 0" range="-1 1"/>
                  <joint name="ball" type="ball"/>
                </body>
              </body>
            </body>
          </worldbody>
        </mujoco>"#;

    fn tree() -> MjcfTree {
        MjcfTree::from_xml(MODEL, Path::new("."), "fallback", None)
            .expect("the test model should parse")
    }

    fn joint<'a>(tree: &'a MjcfTree, name: &str) -> &'a MjcfJoint {
        tree.get_joint_by_name(name)
            .expect("the test model declares this joint")
    }

    #[test]
    fn test_single_hinge() {
        let tree = tree();
        let transform = tree
            .compute_joint_transform(joint(&tree, "yaw"), FRAC_PI_2, false)
            .unwrap();

        assert_eq!(transform.parent_frame, "base");
        assert_eq!(transform.child_frame, "shoulder::yaw");
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(0.0, 0.0, 1.0), 1e-6)
        );
        assert!(
            transform
                .quaternion
                .abs_diff_eq(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), 1e-6)
        );
    }

    /// A hinge with an anchor away from its frame's origin swings that origin around the anchor.
    #[test]
    fn test_off_center_hinge() {
        let tree = tree();
        let transform = tree
            .compute_joint_transform(joint(&tree, "pitch"), FRAC_PI_2, false)
            .unwrap();

        assert_eq!(transform.parent_frame, "shoulder::yaw");
        assert_eq!(transform.child_frame, "shoulder");

        // The origin starts one unit from the anchor at (1, 0, 0), and swings up to (1, 0, 1).
        assert!(
            transform
                .translation
                .abs_diff_eq(Vec3::new(1.0, 0.0, 1.0), 1e-6)
        );
    }

    /// The transforms of the joints of one body chain up to the pose MuJoCo computes for the body.
    #[test]
    fn test_joints_of_one_body_chain() {
        let tree = tree();
        let transforms = tree
            .compute_joint_transforms(&[("pitch", FRAC_PI_2), ("yaw", FRAC_PI_2)], false)
            .unwrap();
        let [pitch, yaw] = transforms.as_slice() else {
            panic!("expected one transform per joint, got {transforms:?}");
        };
        assert_eq!(yaw.child_frame, pitch.parent_frame);

        let rotation = yaw.quaternion * pitch.quaternion;
        let translation = yaw.translation + yaw.quaternion * pitch.translation;

        // Yaw first turns the pitch anchor from (1, 0, 0) to (0, 1, 0), and pitch then rotates about it.
        let expected = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        assert!(rotation.abs_diff_eq(expected, 1e-6));
        assert!(translation.abs_diff_eq(Vec3::new(0.0, 1.0, 2.0), 1e-6));
    }

    /// At their reference values, joints only carry the body's reference pose.
    #[test]
    fn test_reference_values() {
        let tree = tree();
        let yaw = tree
            .compute_joint_transform(joint(&tree, "yaw"), 0.0, false)
            .unwrap();
        let pitch = tree
            .compute_joint_transform(joint(&tree, "pitch"), 0.0, false)
            .unwrap();

        assert!(yaw.translation.abs_diff_eq(Vec3::new(0.0, 0.0, 1.0), 1e-6));
        assert!(pitch.translation.abs_diff_eq(Vec3::ZERO, 1e-6));
        assert!(pitch.quaternion.abs_diff_eq(Quat::IDENTITY, 1e-6));
    }

    /// Slide values are relative to the joint's `ref`, like MuJoCo's `qpos`.
    #[test]
    fn test_slide_reference() {
        let tree = tree();
        let at_reference = tree
            .compute_joint_transform(joint(&tree, "extend"), 0.5, false)
            .unwrap();
        let extended = tree
            .compute_joint_transform(joint(&tree, "extend"), 1.5, false)
            .unwrap();

        assert!(
            at_reference
                .translation
                .abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-6)
        );
        assert!(
            extended
                .translation
                .abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), 1e-6)
        );
    }

    #[test]
    fn test_clamping() {
        let tree = tree();
        let limited = joint(&tree, "limited");

        let clamped = tree.compute_joint_transform(limited, 2.0, true).unwrap();
        assert_eq!(clamped.warnings.len(), 1);
        assert!(clamped.warnings[0].contains("outside limits"));
        assert!(
            clamped
                .quaternion
                .abs_diff_eq(Quat::from_rotation_x(1.0), 1e-6)
        );

        let unclamped = tree.compute_joint_transform(limited, 2.0, false).unwrap();
        assert!(unclamped.warnings.is_empty());
        assert!(
            unclamped
                .quaternion
                .abs_diff_eq(Quat::from_rotation_x(2.0), 1e-6)
        );
    }

    #[test]
    fn test_unsupported_and_unknown_joints() {
        let tree = tree();
        let err = tree
            .compute_joint_transform(joint(&tree, "ball"), 0.0, false)
            .unwrap_err();
        assert!(err.is::<crate::UnsupportedJointType>());

        let err = tree
            .compute_joint_transforms(&[("ball", 0.0)], false)
            .unwrap_err();
        assert!(err.is::<crate::UnsupportedJointType>());

        assert!(
            tree.compute_joint_transforms(&[("nope", 0.0)], false)
                .is_err()
        );
    }

    #[test]
    fn test_joint_of_another_model() {
        let other = MjcfTree::from_xml(
            r#"<mujoco><worldbody><body><joint name="only"/></body></worldbody></mujoco>"#,
            Path::new("."),
            "other",
            None,
        )
        .expect("the test model should parse");

        let tree = tree();
        assert!(
            tree.compute_joint_transform(joint(&other, "only"), 0.0, false)
                .is_err()
        );
        assert!(
            tree.get_visual_geometry_paths(&other.bodies()[1])
                .is_empty()
        );
        assert_eq!(tree.joint_frames(joint(&other, "only")), None);
    }

    /// Every joint has a static transform, so that the frame chain of a body is complete before any joint is logged.
    #[test]
    fn test_static_transforms_chain_frames() {
        let mut chunks = Vec::new();
        tree()
            .emit(&mut |chunk| chunks.push(chunk), &TimePoint::STATIC, true)
            .unwrap();

        let child_frames: Vec<String> = chunks
            .iter()
            .filter(|chunk| chunk.entity_path() == &EntityPath::from("tf_static"))
            .flat_map(|chunk| {
                chunk
                    .iter_slices::<String>(Transform3D::descriptor_child_frame().component)
                    .flatten()
                    .map(|frame| frame.to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(
            child_frames,
            [
                "base",
                "shoulder::yaw",
                "shoulder",
                "slider",
                "wrist::limited",
                "wrist"
            ]
        );
    }

    #[test]
    fn test_frame_prefix() {
        let tree = tree().with_frame_prefix("left/");
        let transform = tree
            .compute_joint_transform(joint(&tree, "yaw"), 0.0, false)
            .unwrap();
        assert_eq!(transform.parent_frame, "left/base");
        assert_eq!(transform.child_frame, "left/shoulder::yaw");
    }
}
