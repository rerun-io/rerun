// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/transform3d.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// A 3D transform.
///
/// ## Example
///
/// ```ignore
/// //! Log some transforms.
///
/// use rerun::{
///     archetypes::{Arrows3D, Transform3D},
///     datatypes::{
///         Angle, RotationAxisAngle, Scale3D, TranslationRotationScale3D,
///     },
///     RecordingStreamBuilder,
/// };
/// use std::f32::consts::PI;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_transform3d").memory()?;
///
///     rec.log(
///         "base",
///         &Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
///     )?;
///
///     rec.log(
///         "base/translated",
///         &Transform3D::new(TranslationRotationScale3D::translation([1.0, 0.0, 0.0])),
///     )?;
///
///     rec.log(
///         "base/translated",
///         &Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
///     )?;
///
///     rec.log(
///         "base/rotated_scaled",
///         &Transform3D::new(TranslationRotationScale3D {
///             rotation: Some(RotationAxisAngle::new([0.0, 0.0, 1.0], Angle::Radians(PI / 4.)).into()),
///             scale: Some(Scale3D::from(2.0)),
///             ..Default::default()
///         }),
///     )?;
///
///     rec.log(
///         "base/rotated_scaled",
///         &Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1200w.png">
///   <img src="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/full.png">
/// </picture>
#[derive(Clone, Debug, PartialEq)]
pub struct Transform3D {
    /// The transform
    pub transform: crate::components::Transform3D,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Transform3D".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Transform3DIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Transform3D".into(),
            "rerun.components.Transform3DIndicator".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl Transform3D {
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`Transform3D`] [`crate::Archetype`]
pub type Transform3DIndicator = crate::GenericIndicatorComponent<Transform3D>;

impl crate::Archetype for Transform3D {
    type Indicator = Transform3DIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.Transform3D".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: Transform3DIndicator = Transform3DIndicator::DEFAULT;
        crate::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let transform = {
            let array = arrays_by_name
                .get("rerun.components.Transform3D")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Transform3D#transform")?;
            <crate::components::Transform3D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Transform3D#transform")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Transform3D#transform")?
        };
        Ok(Self { transform })
    }
}

impl crate::AsComponents for Transform3D {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.transform as &dyn crate::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }
}

impl Transform3D {
    pub fn new(transform: impl Into<crate::components::Transform3D>) -> Self {
        Self {
            transform: transform.into(),
        }
    }
}
