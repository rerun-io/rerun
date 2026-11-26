//! The body tree of a parsed [`mjcf_rs::Model`], and how its geoms are interpreted.

use std::collections::BTreeSet;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use ahash::HashSet;
use anyhow::Context as _;
pub use mjcf_rs::body::JointType as MjcfJointType;
use mjcf_rs::body::{Body, Geom, GeomType};
use re_sdk_types::external::glam::{DQuat, DVec3};

/// MuJoCo's built-in geom color, which a material overrides.
const DEFAULT_GEOM_RGBA: [f64; 4] = [0.5, 0.5, 0.5, 1.0];

/// MuJoCo's built-in material color.
const DEFAULT_MATERIAL_RGBA: [f64; 4] = [1.0; 4];

/// The name of the implicit root body, which is also the root frame.
pub const WORLD_BODY_NAME: &str = "world";

/// MuJoCo's id of a body, which is its index in [`crate::MjcfTree::bodies`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MjcfBodyId(pub usize);

/// MuJoCo's id of a joint, which is its index in [`crate::MjcfTree::joints`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MjcfJointId(pub usize);

/// Whether `text` is an XML document whose root element is `<mujoco>`.
///
/// Only inspects the prolog and the root tag, so it is cheap to call on arbitrary `.xml` files.
pub fn has_mujoco_root(text: &str) -> bool {
    let mut rest = text.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        let skipped = if let Some(after) = rest.strip_prefix("<?") {
            after.split_once("?>")
        } else if let Some(after) = rest.strip_prefix("<!--") {
            after.split_once("-->")
        } else if let Some(after) = rest.strip_prefix("<!") {
            after.split_once('>')
        } else {
            break;
        };
        let Some((_, after)) = skipped else {
            return false;
        };
        rest = after;
    }

    rest.strip_prefix("<mujoco")
        .is_some_and(|after| after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/'))
}

/// A parsed MJCF document, along with its bodies and joints in MuJoCo's id order.
pub struct MjcfModel {
    pub name: String,

    /// Holds the geoms and assets. Its bodies are in the same order as [`Self::bodies`].
    pub source: mjcf_rs::Model,

    /// The directory that relative asset paths are resolved against.
    pub model_dir: PathBuf,

    /// All bodies in depth-first order, so that the index matches MuJoCo's body id.
    ///
    /// Index 0 is the world body, and every other body's parent precedes it.
    pub bodies: Vec<MjcfBody>,

    /// All joints, so that the index matches MuJoCo's joint id.
    pub joints: Vec<MjcfJoint>,
}

/// A rigid body of an MJCF model, which has a frame named after it.
#[derive(Clone, Debug, PartialEq)]
pub struct MjcfBody {
    /// The body's position in MuJoCo's depth-first body order.
    pub id: MjcfBodyId,

    /// Unique among all bodies, and used as the body's frame id.
    ///
    /// Unnamed bodies are called `body_<id>`, and the root is called `world`.
    pub name: String,

    /// The parent body, `None` only for the world body.
    pub parent: Option<MjcfBodyId>,

    /// Pose relative to the parent body, when all joints are at their reference value.
    pub pose: mjcf_rs::Pose,

    /// The joints that move this body relative to its parent, in the order MuJoCo applies them.
    pub joints: Vec<MjcfJointId>,
}

/// A joint of an MJCF model, which moves its body relative to the body's parent.
#[derive(Clone, Debug, PartialEq)]
pub struct MjcfJoint {
    /// The joint's position in MuJoCo's joint order, which follows the body order.
    pub id: MjcfJointId,

    /// Unique among all joints. Unnamed joints are called `joint_<id>`.
    pub name: String,

    /// Hinge and slide joints have one degree of freedom, ball and free joints several.
    pub joint_type: MjcfJointType,

    /// The body this joint moves.
    pub body: MjcfBodyId,

    /// The point the joint rotates about, in the body's frame.
    pub anchor: DVec3,

    /// Unit axis of a hinge or slide joint, in the body's frame.
    pub axis: DVec3,

    /// The value at which the body is in its reference pose (MJCF `ref`).
    pub reference: f64,

    /// The values the joint is limited to, or `None` if it is not limited.
    pub range: Option<RangeInclusive<f64>>,
}

impl MjcfModel {
    /// Parses an MJCF document.
    ///
    /// `model_dir` is the directory of the main model file, which `<include>`s and relative asset
    /// paths are resolved against.
    /// `fallback_name` is used if the model has no `model` attribute.
    pub fn parse(text: &str, model_dir: &Path, fallback_name: &str) -> anyhow::Result<Self> {
        let model = mjcf_rs::Model::from_str(text, model_dir)?;

        let body_names: HashSet<&str> = model
            .bodies
            .iter()
            .filter_map(|entry| entry.body.name.as_deref())
            .collect();
        let joint_names: HashSet<&str> = model
            .bodies
            .iter()
            .flat_map(|entry| &entry.body.joints)
            .filter_map(|joint| joint.name.as_deref())
            .collect();

        let mut bodies = Vec::with_capacity(model.bodies.len());
        let mut joints = Vec::new();

        for (index, entry) in model.bodies.iter().enumerate() {
            let body = &entry.body;

            let mut body_joints = Vec::with_capacity(body.joints.len());
            for joint in &body.joints {
                let joint = convert_joint(
                    &model,
                    MjcfBodyId(index),
                    joint,
                    MjcfJointId(joints.len()),
                    &joint_names,
                )?;
                body_joints.push(joint.id);
                joints.push(joint);
            }

            let mut pose = body.pose;
            pose.rotation = pose.rotation.normalize();
            bodies.push(MjcfBody {
                id: MjcfBodyId(index),
                name: match (&body.name, index) {
                    (Some(name), _) => name.clone(),
                    (None, 0) => WORLD_BODY_NAME.to_owned(),
                    (None, _) => unnamed("body", index, &body_names),
                },
                parent: entry.parent.map(MjcfBodyId),
                pose,
                joints: body_joints,
            });
        }

        Ok(Self {
            name: model
                .name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| fallback_name.to_owned()),
            source: model,
            model_dir: model_dir.to_owned(),
            bodies,
            joints,
        })
    }

    /// Returns `None` if `id` is out of range.
    pub fn body(&self, id: MjcfBodyId) -> Option<&MjcfBody> {
        self.bodies.get(id.0)
    }

    /// Returns `None` if `id` is out of range.
    pub fn joint(&self, id: MjcfJointId) -> Option<&MjcfJoint> {
        self.joints.get(id.0)
    }

    /// The parsed body behind `id`, which holds its geoms.
    pub fn source_body(&self, id: MjcfBodyId) -> Option<&Body> {
        self.source.bodies.get(id.0).map(|entry| &entry.body)
    }
}

/// The name of a model without a `model` attribute.
pub fn model_name_from_path(path: &Path) -> String {
    path.file_stem().map_or_else(
        || "mjcf".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

fn convert_joint(
    model: &mjcf_rs::Model,
    body: MjcfBodyId,
    joint: &mjcf_rs::body::Joint,
    id: MjcfJointId,
    joint_names: &HashSet<&str>,
) -> anyhow::Result<MjcfJoint> {
    let name = joint
        .name
        .clone()
        .unwrap_or_else(|| unnamed("joint", id.0, joint_names));

    let mut axis = DVec3::from_array(joint.axis);
    if matches!(joint.type_, MjcfJointType::Hinge | MjcfJointType::Slide) {
        axis = axis
            .try_normalize()
            .with_context(|| format!("Joint {name:?} has a zero axis"))?;
    }

    let limited = joint
        .limited
        .resolve(model.compiler.autolimits, joint.range.is_some());

    Ok(MjcfJoint {
        id,
        name,
        joint_type: joint.type_,
        body,
        anchor: DVec3::from_array(joint.pos),
        axis,
        reference: joint.ref_,
        range: joint
            .range
            .filter(|_| limited)
            .map(|[lower, upper]| lower..=upper),
    })
}

/// `{kind}_{index}`, extended until it differs from every name in `taken`.
fn unnamed(kind: &str, index: usize, taken: &HashSet<&str>) -> String {
    let mut name = format!("{kind}_{index}");
    while taken.contains(name.as_str()) {
        name.push('_');
    }
    name
}

/// The geoms of `body` that can be visualized, with the name each is logged under.
pub fn drawable_geoms(body: &Body) -> impl Iterator<Item = (String, &Geom)> {
    let geom_names: HashSet<&str> = body
        .geoms
        .iter()
        .filter_map(|geom| geom.name.as_deref())
        .collect();
    body.geoms
        .iter()
        .filter(|geom| match geom.type_ {
            GeomType::Hfield | GeomType::Sdf => false,
            GeomType::Mesh => geom.mesh.is_some(),
            GeomType::Plane
            | GeomType::Sphere
            | GeomType::Capsule
            | GeomType::Ellipsoid
            | GeomType::Cylinder
            | GeomType::Box => true,
        })
        .enumerate()
        .map(move |(index, geom)| {
            let name = geom
                .name
                .clone()
                .unwrap_or_else(|| unnamed("geom", index, &geom_names));
            (name, geom)
        })
}

/// The MJCF name of a geom type.
pub fn geom_type_name(geom_type: GeomType) -> &'static str {
    match geom_type {
        GeomType::Plane => "plane",
        GeomType::Hfield => "hfield",
        GeomType::Sphere => "sphere",
        GeomType::Capsule => "capsule",
        GeomType::Ellipsoid => "ellipsoid",
        GeomType::Cylinder => "cylinder",
        GeomType::Box => "box",
        GeomType::Mesh => "mesh",
        GeomType::Sdf => "sdf",
    }
}

/// The pose of `geom` in its body's frame (with a normalized rotation) and its `size`, with `fromto` applied.
pub fn geom_pose_and_size(geom: &Geom) -> (mjcf_rs::Pose, [f32; 3]) {
    let mut size = geom.size.map(|v| v as f32);
    let mut pose = geom.pose;

    // `fromto` replaces the geom's local pose and its half-length along z.
    // `geom.pose` still holds the enclosing `<frame>`s, which `fromto` is relative to.
    let half_length_index = match geom.type_ {
        GeomType::Capsule | GeomType::Cylinder => Some(1),
        GeomType::Box | GeomType::Ellipsoid => Some(2),
        GeomType::Plane | GeomType::Hfield | GeomType::Sphere | GeomType::Mesh | GeomType::Sdf => {
            None
        }
    };
    if let (Some(index), Some([x0, y0, z0, x1, y1, z1])) = (half_length_index, geom.fromto) {
        let (from, to) = (DVec3::new(x0, y0, z0), DVec3::new(x1, y1, z1));
        let direction = to - from;
        // MuJoCo aligns z with `from - to`, which determines the roll of a box around its axis.
        let rotation = direction.try_normalize().map_or(DQuat::IDENTITY, |dir| {
            DQuat::from_rotation_arc(DVec3::Z, -dir)
        });
        pose *= mjcf_rs::Pose::from_parts((from + to) / 2.0, rotation);
        if index == 2 {
            // With `fromto`, `size[0]` is the half-size along both x and y.
            size[1] = size[0];
        }
        size[index] = (direction.length() / 2.0) as f32;
    }

    pose.rotation = pose.rotation.normalize();
    (pose, size)
}

/// A geom's own color only takes precedence over its material if it differs from MuJoCo's default.
pub fn geom_rgba(model: &mjcf_rs::Model, geom: &Geom, warnings: &mut BTreeSet<String>) -> [f64; 4] {
    let material_rgba = geom.material.as_deref().and_then(|name| {
        let material = model.assets.material(name);
        if material.is_none() {
            warnings.insert(format!("Geom references unknown material {name:?}"));
        }
        material.map(|material| material.rgba.unwrap_or(DEFAULT_MATERIAL_RGBA))
    });

    let geom_rgba = geom.rgba.unwrap_or(DEFAULT_GEOM_RGBA);

    #[expect(
        clippy::float_cmp_const,
        reason = "MuJoCo compares against the default exactly"
    )]
    let use_material = geom_rgba == DEFAULT_GEOM_RGBA;
    material_rgba.filter(|_| use_material).unwrap_or(geom_rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> MjcfModel {
        MjcfModel::parse(xml, Path::new("."), "fallback").expect("the test model should parse")
    }

    #[test]
    fn test_has_mujoco_root() {
        assert!(has_mujoco_root(r#"<mujoco model="a"/>"#));
        assert!(has_mujoco_root(
            "\u{feff}<?xml version=\"1.0\"?>\n<!-- comment -->\n<mujoco>"
        ));
        assert!(!has_mujoco_root("<robot name=\"urdf\"/>"));
        assert!(!has_mujoco_root("<mujocoextra/>"));
        assert!(!has_mujoco_root("<!-- unterminated"));
    }

    #[test]
    fn test_material_color_precedence() {
        let model = parse(
            r#"
            <mujoco>
              <asset>
                <material name="blue" rgba="0 0 1 1"/>
                <material name="untinted"/>
              </asset>
              <worldbody>
                <geom material="blue"/>
                <geom material="blue" rgba="0.5 0.5 0.5 1"/>
                <geom material="blue" rgba="0 1 0 1"/>
                <geom material="untinted"/>
                <geom/>
              </worldbody>
            </mujoco>"#,
        );

        let mut warnings = BTreeSet::new();
        let colors = model.source.bodies[0]
            .body
            .geoms
            .iter()
            .map(|geom| geom_rgba(&model.source, geom, &mut warnings))
            .collect::<Vec<_>>();
        assert!(warnings.is_empty());
        assert_eq!(
            colors,
            [
                [0.0, 0.0, 1.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
                [0.5, 0.5, 0.5, 1.0],
            ]
        );
    }

    #[test]
    fn test_fromto_inside_frame() {
        let model = parse(
            r#"
            <mujoco>
              <compiler angle="radian"/>
              <worldbody>
                <body name="b">
                  <frame pos="1 0 0" euler="0 0 1.5707963267948966">
                    <geom type="capsule" size="0.1" fromto="0 0 0 2 0 0"/>
                  </frame>
                </body>
              </worldbody>
            </mujoco>"#,
        );

        let (pose, [radius, half_length, _]) =
            geom_pose_and_size(&model.source.bodies[1].body.geoms[0]);
        assert!((radius - 0.1).abs() < 1e-6);
        assert!((half_length - 1.0).abs() < 1e-6);
        // The capsule runs along the frame's x axis, which is the body's y axis.
        assert!(
            pose.translation
                .abs_diff_eq(DVec3::new(1.0, 1.0, 0.0), 1e-9)
        );
        assert!((pose.rotation * DVec3::Z).abs_diff_eq(-DVec3::Y, 1e-9));
    }

    #[test]
    fn test_fromto_box() {
        let model = parse(
            r#"
            <mujoco>
              <worldbody>
                <geom type="box" size="0.1" fromto="0 0 0 0 0 2"/>
              </worldbody>
            </mujoco>"#,
        );

        let (pose, size) = geom_pose_and_size(&model.source.bodies[0].body.geoms[0]);
        assert_eq!(size, [0.1, 0.1, 1.0]);
        assert!(
            pose.translation
                .abs_diff_eq(DVec3::new(0.0, 0.0, 1.0), 1e-9)
        );
    }

    #[test]
    fn test_fromto_rotation_matches_mujoco() {
        let model = parse(
            r#"
            <mujoco>
              <worldbody>
                <geom type="box" size="0.1" fromto="0 0 0 1 2 3"/>
              </worldbody>
            </mujoco>"#,
        );

        let (pose, _) = geom_pose_and_size(&model.source.bodies[0].body.geoms[0]);
        let expected = DQuat::from_xyzw(0.84894846, -0.42447423, 0.0, 0.31481445);
        assert!(pose.rotation.abs_diff_eq(expected, 1e-6));
    }

    #[test]
    fn test_unnamed_bodies_get_unique_names() {
        let model = parse(
            r#"
            <mujoco model="named">
              <worldbody><body><body/></body><body name="body_9"/></worldbody>
            </mujoco>"#,
        );

        assert_eq!(model.name, "named");
        let names = model
            .bodies
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["world", "body_1", "body_2", "body_9"]);
        assert_eq!(model.bodies[2].parent, Some(MjcfBodyId(1)));
    }

    #[test]
    fn test_generated_names_avoid_declared_names() {
        let model = parse(
            r#"
            <mujoco>
              <worldbody>
                <body>
                  <geom size="0.1"/>
                  <geom name="geom_0" size="0.1"/>
                </body>
                <body name="body_1">
                  <joint/>
                  <joint name="joint_0"/>
                </body>
              </worldbody>
            </mujoco>"#,
        );

        let names = |iter: &mut dyn Iterator<Item = String>| iter.collect::<Vec<_>>();
        assert_eq!(
            names(&mut model.bodies.iter().map(|b| b.name.clone())),
            ["world", "body_1_", "body_1"]
        );
        assert_eq!(
            names(&mut model.joints.iter().map(|j| j.name.clone())),
            ["joint_0_", "joint_0"]
        );
        assert_eq!(
            names(&mut drawable_geoms(&model.source.bodies[1].body).map(|(name, _)| name)),
            ["geom_0_", "geom_0"]
        );
    }

    /// MJCF angles are in degrees unless the compiler says otherwise, but joint values are always in radians.
    #[test]
    fn test_default_angle_unit_is_degree() {
        let model = parse(
            r#"
            <mujoco>
              <worldbody><body><joint range="-90 90" ref="45"/></body></worldbody>
            </mujoco>"#,
        );

        let joint = &model.joints[0];
        let range = joint
            .range
            .clone()
            .expect("a joint with a range is limited");
        assert!((range.start() + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!((range.end() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!((joint.reference - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    }
}
