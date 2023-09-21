// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/clear.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// Empties all the components of an entity.
///
/// ## Examples
///
/// Flat:
/// ```ignore
/// //! Log a batch of 3D arrows.
///
/// use rerun::{
///     archetypes::{Arrows3D, Clear},
///     components::Color,
///     external::glam,
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_clear_simple").memory()?;
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
///             &Arrows3D::new([vector])
///                 .with_origins([origin])
///                 .with_colors([Color::from_rgb(color.0, color.1, color.2)]),
///         )?;
///     }
///
///     // Now clear them, one by one on each tick.
///     for i in 0..vectors.len() {
///         rec.log(format!("arrows/{i}"), &Clear::flat())?;
///     }
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
///
/// Recursive:
/// ```ignore
/// //! Log a batch of 3D arrows.
///
/// use rerun::{
///     archetypes::{Arrows3D, Clear},
///     components::Color,
///     external::glam,
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_clear_recursive").memory()?;
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
///             &Arrows3D::new([vector])
///                 .with_origins([origin])
///                 .with_colors([Color::from_rgb(color.0, color.1, color.2)]),
///         )?;
///     }
///
///     // Now clear all of them at once.
///     rec.log("arrows", &Clear::recursive())?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clear {
    pub settings: crate::components::ClearSettings,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ClearSettings".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.ClearIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.ClearSettings".into(),
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
    fn num_instances(&self) -> usize {
        1
    }

    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        [
            Some(Self::Indicator::batch(self.num_instances() as _).into()),
            Some((&self.settings as &dyn crate::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        use crate::{Loggable as _, ResultExt as _};
        Ok([{
            Some({
                let array = <crate::components::ClearSettings>::try_to_arrow([&self.settings]);
                array.map(|array| {
                    let datatype = ::arrow2::datatypes::DataType::Extension(
                        "rerun.components.ClearSettings".into(),
                        Box::new(array.data_type().clone()),
                        None,
                    );
                    (
                        ::arrow2::datatypes::Field::new("settings", datatype, false),
                        array,
                    )
                })
            })
            .transpose()
            .with_context("rerun.archetypes.Clear#settings")?
        }]
        .into_iter()
        .flatten()
        .collect())
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let settings = {
            let array = arrays_by_name
                .get("settings")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Clear#settings")?;
            <crate::components::ClearSettings>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Clear#settings")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Clear#settings")?
        };
        Ok(Self { settings })
    }
}

impl Clear {
    pub fn new(settings: impl Into<crate::components::ClearSettings>) -> Self {
        Self {
            settings: settings.into(),
        }
    }
}
