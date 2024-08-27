// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/ellipsoids3d.fbs".

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

/// **Archetype**: 3D ellipsoids or spheres.
///
/// This archetype is for ellipsoids or spheres whose size is a key part of the data
/// (e.g. a bounding sphere).
/// For points whose radii are for the sake of visualization, use [`archetypes::Points3D`][crate::archetypes::Points3D] instead.
///
/// Note that orienting and placing the ellipsoids/spheres is handled via `[archetypes.InstancePoses3D]`.
/// Some of its component are repeated here for convenience.
/// If there's more instance poses than half sizes, the last half size will be repeated for the remaining poses.
#[derive(Clone, Debug, PartialEq)]
pub struct Ellipsoids3D {
    /// For each ellipsoid, half of its size on its three axes.
    ///
    /// If all components are equal, then it is a sphere with that radius.
    pub half_sizes: Vec<crate::components::HalfSize3D>,

    /// Optional center positions of the ellipsoids.
    ///
    /// If not specified, the centers will be at (0, 0, 0).
    /// Note that this uses a [`components::PoseTranslation3D`][crate::components::PoseTranslation3D] which is also used by [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
    pub centers: Option<Vec<crate::components::PoseTranslation3D>>,

    /// Rotations via axis + angle.
    ///
    /// If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    /// Note that this uses a [`components::PoseRotationAxisAngle`][crate::components::PoseRotationAxisAngle] which is also used by [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
    pub rotation_axis_angles: Option<Vec<crate::components::PoseRotationAxisAngle>>,

    /// Rotations via quaternion.
    ///
    /// If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    /// Note that this uses a [`components::PoseRotationQuat`][crate::components::PoseRotationQuat] which is also used by [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
    pub quaternions: Option<Vec<crate::components::PoseRotationQuat>>,

    /// Optional colors for the ellipsoids.
    pub colors: Option<Vec<crate::components::Color>>,

    /// Optional radii for the lines used when the ellipsoid is rendered as a wireframe.
    pub line_radii: Option<Vec<crate::components::Radius>>,

    /// Optionally choose whether the ellipsoids are drawn with lines or solid.
    pub fill_mode: Option<crate::components::FillMode>,

    /// Optional text labels for the ellipsoids.
    pub labels: Option<Vec<crate::components::Text>>,

    /// Optional choice of whether the text labels should be shown by default.
    pub show_labels: Option<crate::components::ShowLabels>,

    /// Optional class ID for the ellipsoids.
    ///
    /// The class ID provides colors and labels if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,
}

impl ::re_types_core::SizeBytes for Ellipsoids3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.half_sizes.heap_size_bytes()
            + self.centers.heap_size_bytes()
            + self.rotation_axis_angles.heap_size_bytes()
            + self.quaternions.heap_size_bytes()
            + self.colors.heap_size_bytes()
            + self.line_radii.heap_size_bytes()
            + self.fill_mode.heap_size_bytes()
            + self.labels.heap_size_bytes()
            + self.show_labels.heap_size_bytes()
            + self.class_ids.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::components::HalfSize3D>>::is_pod()
            && <Option<Vec<crate::components::PoseTranslation3D>>>::is_pod()
            && <Option<Vec<crate::components::PoseRotationAxisAngle>>>::is_pod()
            && <Option<Vec<crate::components::PoseRotationQuat>>>::is_pod()
            && <Option<Vec<crate::components::Color>>>::is_pod()
            && <Option<Vec<crate::components::Radius>>>::is_pod()
            && <Option<crate::components::FillMode>>::is_pod()
            && <Option<Vec<crate::components::Text>>>::is_pod()
            && <Option<crate::components::ShowLabels>>::is_pod()
            && <Option<Vec<crate::components::ClassId>>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.HalfSize3D".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.PoseTranslation3D".into(),
            "rerun.components.Color".into(),
            "rerun.components.Ellipsoids3DIndicator".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 7usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.PoseRotationAxisAngle".into(),
            "rerun.components.PoseRotationQuat".into(),
            "rerun.components.Radius".into(),
            "rerun.components.FillMode".into(),
            "rerun.components.Text".into(),
            "rerun.components.ShowLabels".into(),
            "rerun.components.ClassId".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 11usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.HalfSize3D".into(),
            "rerun.components.PoseTranslation3D".into(),
            "rerun.components.Color".into(),
            "rerun.components.Ellipsoids3DIndicator".into(),
            "rerun.components.PoseRotationAxisAngle".into(),
            "rerun.components.PoseRotationQuat".into(),
            "rerun.components.Radius".into(),
            "rerun.components.FillMode".into(),
            "rerun.components.Text".into(),
            "rerun.components.ShowLabels".into(),
            "rerun.components.ClassId".into(),
        ]
    });

impl Ellipsoids3D {
    /// The total number of components in the archetype: 1 required, 3 recommended, 7 optional
    pub const NUM_COMPONENTS: usize = 11usize;
}

/// Indicator component for the [`Ellipsoids3D`] [`::re_types_core::Archetype`]
pub type Ellipsoids3DIndicator = ::re_types_core::GenericIndicatorComponent<Ellipsoids3D>;

impl ::re_types_core::Archetype for Ellipsoids3D {
    type Indicator = Ellipsoids3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Ellipsoids3D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Ellipsoids 3D"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: Ellipsoids3DIndicator = Ellipsoids3DIndicator::DEFAULT;
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
        let half_sizes = {
            let array = arrays_by_name
                .get("rerun.components.HalfSize3D")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Ellipsoids3D#half_sizes")?;
            <crate::components::HalfSize3D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Ellipsoids3D#half_sizes")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.Ellipsoids3D#half_sizes")?
        };
        let centers = if let Some(array) = arrays_by_name.get("rerun.components.PoseTranslation3D")
        {
            Some({
                <crate::components::PoseTranslation3D>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Ellipsoids3D#centers")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Ellipsoids3D#centers")?
            })
        } else {
            None
        };
        let rotation_axis_angles =
            if let Some(array) = arrays_by_name.get("rerun.components.PoseRotationAxisAngle") {
                Some({
                    <crate::components::PoseRotationAxisAngle>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.Ellipsoids3D#rotation_axis_angles")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.Ellipsoids3D#rotation_axis_angles")?
                })
            } else {
                None
            };
        let quaternions =
            if let Some(array) = arrays_by_name.get("rerun.components.PoseRotationQuat") {
                Some({
                    <crate::components::PoseRotationQuat>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.Ellipsoids3D#quaternions")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.Ellipsoids3D#quaternions")?
                })
            } else {
                None
            };
        let colors = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Ellipsoids3D#colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Ellipsoids3D#colors")?
            })
        } else {
            None
        };
        let line_radii = if let Some(array) = arrays_by_name.get("rerun.components.Radius") {
            Some({
                <crate::components::Radius>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Ellipsoids3D#line_radii")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Ellipsoids3D#line_radii")?
            })
        } else {
            None
        };
        let fill_mode = if let Some(array) = arrays_by_name.get("rerun.components.FillMode") {
            <crate::components::FillMode>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Ellipsoids3D#fill_mode")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let labels = if let Some(array) = arrays_by_name.get("rerun.components.Text") {
            Some({
                <crate::components::Text>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Ellipsoids3D#labels")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Ellipsoids3D#labels")?
            })
        } else {
            None
        };
        let show_labels = if let Some(array) = arrays_by_name.get("rerun.components.ShowLabels") {
            <crate::components::ShowLabels>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Ellipsoids3D#show_labels")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_name.get("rerun.components.ClassId") {
            Some({
                <crate::components::ClassId>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Ellipsoids3D#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Ellipsoids3D#class_ids")?
            })
        } else {
            None
        };
        Ok(Self {
            half_sizes,
            centers,
            rotation_axis_angles,
            quaternions,
            colors,
            line_radii,
            fill_mode,
            labels,
            show_labels,
            class_ids,
        })
    }
}

impl ::re_types_core::AsComponents for Ellipsoids3D {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.half_sizes as &dyn ComponentBatch).into()),
            self.centers
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.rotation_axis_angles
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.quaternions
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.colors
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.line_radii
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.fill_mode
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.labels
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.show_labels
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.class_ids
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl Ellipsoids3D {
    /// Create a new `Ellipsoids3D`.
    #[inline]
    pub(crate) fn new(
        half_sizes: impl IntoIterator<Item = impl Into<crate::components::HalfSize3D>>,
    ) -> Self {
        Self {
            half_sizes: half_sizes.into_iter().map(Into::into).collect(),
            centers: None,
            rotation_axis_angles: None,
            quaternions: None,
            colors: None,
            line_radii: None,
            fill_mode: None,
            labels: None,
            show_labels: None,
            class_ids: None,
        }
    }

    /// Optional center positions of the ellipsoids.
    ///
    /// If not specified, the centers will be at (0, 0, 0).
    /// Note that this uses a [`components::PoseTranslation3D`][crate::components::PoseTranslation3D] which is also used by [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
    #[inline]
    pub fn with_centers(
        mut self,
        centers: impl IntoIterator<Item = impl Into<crate::components::PoseTranslation3D>>,
    ) -> Self {
        self.centers = Some(centers.into_iter().map(Into::into).collect());
        self
    }

    /// Rotations via axis + angle.
    ///
    /// If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    /// Note that this uses a [`components::PoseRotationAxisAngle`][crate::components::PoseRotationAxisAngle] which is also used by [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
    #[inline]
    pub fn with_rotation_axis_angles(
        mut self,
        rotation_axis_angles: impl IntoIterator<
            Item = impl Into<crate::components::PoseRotationAxisAngle>,
        >,
    ) -> Self {
        self.rotation_axis_angles =
            Some(rotation_axis_angles.into_iter().map(Into::into).collect());
        self
    }

    /// Rotations via quaternion.
    ///
    /// If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    /// Note that this uses a [`components::PoseRotationQuat`][crate::components::PoseRotationQuat] which is also used by [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D].
    #[inline]
    pub fn with_quaternions(
        mut self,
        quaternions: impl IntoIterator<Item = impl Into<crate::components::PoseRotationQuat>>,
    ) -> Self {
        self.quaternions = Some(quaternions.into_iter().map(Into::into).collect());
        self
    }

    /// Optional colors for the ellipsoids.
    #[inline]
    pub fn with_colors(
        mut self,
        colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }

    /// Optional radii for the lines used when the ellipsoid is rendered as a wireframe.
    #[inline]
    pub fn with_line_radii(
        mut self,
        line_radii: impl IntoIterator<Item = impl Into<crate::components::Radius>>,
    ) -> Self {
        self.line_radii = Some(line_radii.into_iter().map(Into::into).collect());
        self
    }

    /// Optionally choose whether the ellipsoids are drawn with lines or solid.
    #[inline]
    pub fn with_fill_mode(mut self, fill_mode: impl Into<crate::components::FillMode>) -> Self {
        self.fill_mode = Some(fill_mode.into());
        self
    }

    /// Optional text labels for the ellipsoids.
    #[inline]
    pub fn with_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<crate::components::Text>>,
    ) -> Self {
        self.labels = Some(labels.into_iter().map(Into::into).collect());
        self
    }

    /// Optional choice of whether the text labels should be shown by default.
    #[inline]
    pub fn with_show_labels(
        mut self,
        show_labels: impl Into<crate::components::ShowLabels>,
    ) -> Self {
        self.show_labels = Some(show_labels.into());
        self
    }

    /// Optional class ID for the ellipsoids.
    ///
    /// The class ID provides colors and labels if not specified explicitly.
    #[inline]
    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = Some(class_ids.into_iter().map(Into::into).collect());
        self
    }
}
