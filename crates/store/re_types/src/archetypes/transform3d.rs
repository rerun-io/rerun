// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/transform3d.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: A transform between two 3D spaces, i.e. a pose.
///
/// From the point of view of the entity's coordinate system,
/// all components are applied in the inverse order they are listed here.
/// E.g. if both a translation and a max3x3 transform are present,
/// the 3x3 matrix is applied first, followed by the translation.
///
/// Whenever you log this archetype, it will write all components, even if you do not explicitly set them.
/// This means that if you first log a transform with only a translation, and then log one with only a rotation,
/// i will be resolved to a transform with only a rotation.
///
/// ## Examples
///
/// ### Variety of 3D transforms
/// ```ignore
/// use std::f32::consts::TAU;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d").spawn()?;
///
///     let arrow = rerun::Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]);
///
///     rec.log("base", &arrow)?;
///
///     rec.log(
///         "base/translated",
///         &rerun::Transform3D::from_translation([1.0, 0.0, 0.0]),
///     )?;
///
///     rec.log("base/translated", &arrow)?;
///
///     rec.log(
///         "base/rotated_scaled",
///         &rerun::Transform3D::from_rotation_scale(
///             rerun::RotationAxisAngle::new([0.0, 0.0, 1.0], rerun::Angle::from_radians(TAU / 8.0)),
///             rerun::Scale3D::from(2.0),
///         ),
///     )?;
///
///     rec.log("base/rotated_scaled", &arrow)?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1200w.png">
///   <img src="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/full.png" width="640">
/// </picture>
/// </center>
///
/// ### Transform hierarchy
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d_hierarchy").spawn()?;
///
///     // TODO(#5521): log two space views as in the python example
///
///     rec.set_time_seconds("sim_time", 0.0);
///
///     // Planetary motion is typically in the XY plane.
///     rec.log_static("/", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP)?;
///
///     // Setup points, all are in the center of their own space:
///     rec.log(
///         "sun",
///         &rerun::Points3D::new([[0.0, 0.0, 0.0]])
///             .with_radii([1.0])
///             .with_colors([rerun::Color::from_rgb(255, 200, 10)]),
///     )?;
///     rec.log(
///         "sun/planet",
///         &rerun::Points3D::new([[0.0, 0.0, 0.0]])
///             .with_radii([0.4])
///             .with_colors([rerun::Color::from_rgb(40, 80, 200)]),
///     )?;
///     rec.log(
///         "sun/planet/moon",
///         &rerun::Points3D::new([[0.0, 0.0, 0.0]])
///             .with_radii([0.15])
///             .with_colors([rerun::Color::from_rgb(180, 180, 180)]),
///     )?;
///
///     // Draw fixed paths where the planet & moon move.
///     let d_planet = 6.0;
///     let d_moon = 3.0;
///     let angles = (0..=100).map(|i| i as f32 * 0.01 * std::f32::consts::TAU);
///     let circle: Vec<_> = angles.map(|angle| [angle.sin(), angle.cos()]).collect();
///     rec.log(
///         "sun/planet_path",
///         &rerun::LineStrips3D::new([rerun::LineStrip3D::from_iter(
///             circle
///                 .iter()
///                 .map(|p| [p[0] * d_planet, p[1] * d_planet, 0.0]),
///         )]),
///     )?;
///     rec.log(
///         "sun/planet/moon_path",
///         &rerun::LineStrips3D::new([rerun::LineStrip3D::from_iter(
///             circle.iter().map(|p| [p[0] * d_moon, p[1] * d_moon, 0.0]),
///         )]),
///     )?;
///
///     // Movement via transforms.
///     for i in 0..(6 * 120) {
///         let time = i as f32 / 120.0;
///         rec.set_time_seconds("sim_time", time);
///         let r_moon = time * 5.0;
///         let r_planet = time * 2.0;
///
///         rec.log(
///             "sun/planet",
///             &rerun::Transform3D::from_translation_rotation(
///                 [r_planet.sin() * d_planet, r_planet.cos() * d_planet, 0.0],
///                 rerun::RotationAxisAngle {
///                     axis: [1.0, 0.0, 0.0].into(),
///                     angle: rerun::Angle::from_degrees(20.0),
///                 },
///             ),
///         )?;
///         rec.log(
///             "sun/planet/moon",
///             &rerun::Transform3D::from_translation([
///                 r_moon.cos() * d_moon,
///                 r_moon.sin() * d_moon,
///                 0.0,
///             ])
///             .with_relation(rerun::TransformRelation::ChildFromParent),
///         )?;
///     }
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/1200w.png">
///   <img src="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct Transform3D {
    /// Translation vector.
    pub translation: Option<crate::components::Translation3D>,

    /// Rotation via axis + angle.
    pub rotation_axis_angle: Option<crate::components::RotationAxisAngle>,

    /// Rotation via quaternion.
    pub quaternion: Option<crate::components::RotationQuat>,

    /// Scaling factor.
    pub scale: Option<crate::components::Scale3D>,

    /// 3x3 transformation matrix.
    pub mat3x3: Option<crate::components::TransformMat3x3>,

    /// Specifies the relation this transform establishes between this entity and its parent.
    pub relation: Option<crate::components::TransformRelation>,

    /// Visual length of the 3 axes.
    ///
    /// The length is interpreted in the local coordinate system of the transform.
    /// If the transform is scaled, the axes will be scaled accordingly.
    pub axis_length: Option<crate::components::AxisLength>,
}

impl ::re_types_core::SizeBytes for Transform3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.translation.heap_size_bytes()
            + self.rotation_axis_angle.heap_size_bytes()
            + self.quaternion.heap_size_bytes()
            + self.scale.heap_size_bytes()
            + self.mat3x3.heap_size_bytes()
            + self.relation.heap_size_bytes()
            + self.axis_length.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::components::Translation3D>>::is_pod()
            && <Option<crate::components::RotationAxisAngle>>::is_pod()
            && <Option<crate::components::RotationQuat>>::is_pod()
            && <Option<crate::components::Scale3D>>::is_pod()
            && <Option<crate::components::TransformMat3x3>>::is_pod()
            && <Option<crate::components::TransformRelation>>::is_pod()
            && <Option<crate::components::AxisLength>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Transform3DIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 7usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Translation3D".into(),
            "rerun.components.RotationAxisAngle".into(),
            "rerun.components.RotationQuat".into(),
            "rerun.components.Scale3D".into(),
            "rerun.components.TransformMat3x3".into(),
            "rerun.components.TransformRelation".into(),
            "rerun.components.AxisLength".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 8usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Transform3DIndicator".into(),
            "rerun.components.Translation3D".into(),
            "rerun.components.RotationAxisAngle".into(),
            "rerun.components.RotationQuat".into(),
            "rerun.components.Scale3D".into(),
            "rerun.components.TransformMat3x3".into(),
            "rerun.components.TransformRelation".into(),
            "rerun.components.AxisLength".into(),
        ]
    });

impl Transform3D {
    /// The total number of components in the archetype: 0 required, 1 recommended, 7 optional
    pub const NUM_COMPONENTS: usize = 8usize;
}

/// Indicator component for the [`Transform3D`] [`::re_types_core::Archetype`]
pub type Transform3DIndicator = ::re_types_core::GenericIndicatorComponent<Transform3D>;

impl ::re_types_core::Archetype for Transform3D {
    type Indicator = Transform3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Transform3D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Transform 3D"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: Transform3DIndicator = Transform3DIndicator::DEFAULT;
        MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let translation = if let Some(array) = arrays_by_name.get("rerun.components.Translation3D")
        {
            <crate::components::Translation3D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#translation")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let rotation_axis_angle =
            if let Some(array) = arrays_by_name.get("rerun.components.RotationAxisAngle") {
                <crate::components::RotationAxisAngle>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Transform3D#rotation_axis_angle")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let quaternion = if let Some(array) = arrays_by_name.get("rerun.components.RotationQuat") {
            <crate::components::RotationQuat>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#quaternion")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let scale = if let Some(array) = arrays_by_name.get("rerun.components.Scale3D") {
            <crate::components::Scale3D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#scale")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let mat3x3 = if let Some(array) = arrays_by_name.get("rerun.components.TransformMat3x3") {
            <crate::components::TransformMat3x3>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#mat3x3")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let relation = if let Some(array) = arrays_by_name.get("rerun.components.TransformRelation")
        {
            <crate::components::TransformRelation>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#relation")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let axis_length = if let Some(array) = arrays_by_name.get("rerun.components.AxisLength") {
            <crate::components::AxisLength>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#axis_length")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self {
            translation,
            rotation_axis_angle,
            quaternion,
            scale,
            mat3x3,
            relation,
            axis_length,
        })
    }
}

impl ::re_types_core::AsComponents for Transform3D {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.translation as &dyn ComponentBatch).into()),
            Some((&self.rotation_axis_angle as &dyn ComponentBatch).into()),
            Some((&self.quaternion as &dyn ComponentBatch).into()),
            Some((&self.scale as &dyn ComponentBatch).into()),
            Some((&self.mat3x3 as &dyn ComponentBatch).into()),
            Some((&self.relation as &dyn ComponentBatch).into()),
            Some((&self.axis_length as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for Transform3D {}

impl Transform3D {
    /// Create a new `Transform3D` which when logged will clear the values of all components.
    #[inline]
    pub fn clear() -> Self {
        Self {
            translation: None,
            rotation_axis_angle: None,
            quaternion: None,
            scale: None,
            mat3x3: None,
            relation: None,
            axis_length: None,
        }
    }

    /// Translation vector.
    #[inline]
    pub fn with_translation(
        mut self,
        translation: impl Into<crate::components::Translation3D>,
    ) -> Self {
        self.translation = Some(translation.into());
        self
    }

    /// Rotation via axis + angle.
    #[inline]
    pub fn with_rotation_axis_angle(
        mut self,
        rotation_axis_angle: impl Into<crate::components::RotationAxisAngle>,
    ) -> Self {
        self.rotation_axis_angle = Some(rotation_axis_angle.into());
        self
    }

    /// Rotation via quaternion.
    #[inline]
    pub fn with_quaternion(
        mut self,
        quaternion: impl Into<crate::components::RotationQuat>,
    ) -> Self {
        self.quaternion = Some(quaternion.into());
        self
    }

    /// Scaling factor.
    #[inline]
    pub fn with_scale(mut self, scale: impl Into<crate::components::Scale3D>) -> Self {
        self.scale = Some(scale.into());
        self
    }

    /// 3x3 transformation matrix.
    #[inline]
    pub fn with_mat3x3(mut self, mat3x3: impl Into<crate::components::TransformMat3x3>) -> Self {
        self.mat3x3 = Some(mat3x3.into());
        self
    }

    /// Specifies the relation this transform establishes between this entity and its parent.
    #[inline]
    pub fn with_relation(
        mut self,
        relation: impl Into<crate::components::TransformRelation>,
    ) -> Self {
        self.relation = Some(relation.into());
        self
    }

    /// Visual length of the 3 axes.
    ///
    /// The length is interpreted in the local coordinate system of the transform.
    /// If the transform is scaled, the axes will be scaled accordingly.
    #[inline]
    pub fn with_axis_length(
        mut self,
        axis_length: impl Into<crate::components::AxisLength>,
    ) -> Self {
        self.axis_length = Some(axis_length.into());
        self
    }
}
