// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/clear.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
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

use crate::external::arrow2;
use crate::ComponentName;
use crate::SerializationResult;
use crate::{ComponentBatch, MaybeOwnedComponentBatch};
use crate::{DeserializationError, DeserializationResult};

/// **Archetype**: Empties all the components of an entity.
///
/// ## Example
///
/// ### Flat
/// ```ignore
/// use rerun::external::glam;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_clear_simple").spawn()?;
///
///     #[rustfmt::skip]
///     let (vectors, origins, colors) = (
///         [glam::Vec3::X,    glam::Vec3::NEG_Y, glam::Vec3::NEG_X, glam::Vec3::Y],
///         [(-0.5, 0.5, 0.0), (0.5, 0.5, 0.0),   (0.5, -0.5, 0.0),  (-0.5, -0.5, 0.0)],
///         [(200, 0, 0),      (0, 200, 0),       (0, 0, 200),       (200, 0, 200)],
///     );
///
///     // Log a handful of arrows.
///     for (i, ((vector, origin), color)) in vectors.into_iter().zip(origins).zip(colors).enumerate() {
///         rec.log(
///             format!("arrows/{i}"),
///             &rerun::Arrows3D::from_vectors([vector])
///                 .with_origins([origin])
///                 .with_colors([rerun::Color::from_rgb(color.0, color.1, color.2)]),
///         )?;
///     }
///
///     // Now clear them, one by one on each tick.
///     for i in 0..vectors.len() {
///         rec.log(format!("arrows/{i}"), &rerun::Clear::flat())?;
///     }
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/1200w.png">
///   <img src="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clear {
    pub is_recursive: crate::components::ClearIsRecursive,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ClearIsRecursive".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ClearIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ClearIsRecursive".into(),
            "rerun.components.ClearIndicator".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl Clear {
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`Clear`] [`crate::Archetype`]
pub type ClearIndicator = crate::GenericIndicatorComponent<Clear>;

impl crate::Archetype for Clear {
    type Indicator = ClearIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.Clear".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: ClearIndicator = ClearIndicator::DEFAULT;
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
    fn from_arrow(
        arrow_data: impl IntoIterator<Item = (arrow2::datatypes::Field, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let is_recursive = {
            let array = arrays_by_name
                .get("rerun.components.ClearIsRecursive")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Clear#is_recursive")?;
            <crate::components::ClearIsRecursive>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Clear#is_recursive")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Clear#is_recursive")?
        };
        Ok(Self { is_recursive })
    }
}

impl crate::AsComponents for Clear {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.is_recursive as &dyn ComponentBatch).into()),
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

impl Clear {
    pub fn new(is_recursive: impl Into<crate::components::ClearIsRecursive>) -> Self {
        Self {
            is_recursive: is_recursive.into(),
        }
    }
}
