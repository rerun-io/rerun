// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/view_coordinates.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: How we interpret the coordinate system of an entity/space.
///
/// For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?
///
/// The three coordinates are always ordered as [x, y, z].
///
/// For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
/// down, and the Z axis points forward.
///
/// ## Example
///
/// ### View coordinates for adjusting the eye camera
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_view_coordinates").spawn()?;
///
///     rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
///     rec.log(
///         "world/xyz",
///         &rerun::Arrows3D::from_vectors(
///             [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], //
///         )
///         .with_colors([[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/1200w.png">
///   <img src="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct ViewCoordinates {
    pub xyz: crate::components::ViewCoordinates,
}

impl ::re_types_core::SizeBytes for ViewCoordinates {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.xyz.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::ViewCoordinates>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ViewCoordinates".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ViewCoordinatesIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ViewCoordinates".into(),
            "rerun.components.ViewCoordinatesIndicator".into(),
        ]
    });

impl ViewCoordinates {
    pub const NUM_COMPONENTS: usize = 2usize;
}

/// Indicator component for the [`ViewCoordinates`] [`::re_types_core::Archetype`]
pub type ViewCoordinatesIndicator = ::re_types_core::GenericIndicatorComponent<ViewCoordinates>;

impl ::re_types_core::Archetype for ViewCoordinates {
    type Indicator = ViewCoordinatesIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.ViewCoordinates".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: ViewCoordinatesIndicator = ViewCoordinatesIndicator::DEFAULT;
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
        let xyz = {
            let array = arrays_by_name
                .get("rerun.components.ViewCoordinates")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.ViewCoordinates#xyz")?;
            <crate::components::ViewCoordinates>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.ViewCoordinates#xyz")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.ViewCoordinates#xyz")?
        };
        Ok(Self { xyz })
    }
}

impl ::re_types_core::AsComponents for ViewCoordinates {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.xyz as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ViewCoordinates {
    pub fn new(xyz: impl Into<crate::components::ViewCoordinates>) -> Self {
        Self { xyz: xyz.into() }
    }
}
