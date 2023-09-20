// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/disconnected_space.fbs".

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

/// Specifies that the entity path at which this is logged is disconnected from its parent.
///
/// This is useful for specifying that a subgraph is independent of the rest of the scene.
///
/// If a transform or pinhole is logged on the same path, this archetype's components
/// will be ignored.
///
/// ## Example
///
/// ```ignore
/// //! Disconnect two spaces.
///
/// use rerun::{
///     archetypes::{DisconnectedSpace, Points3D},
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         RecordingStreamBuilder::new("rerun_example_disconnected_space").memory()?;
///
///     // These two points can be projected into the same space..
///     rec.log("world/room1/point", &Points3D::new([(0.0, 0.0, 0.0)]))?;
///     rec.log("world/room2/point", &Points3D::new([(1.0, 1.0, 1.0)]))?;
///
///     // ..but this one lives in a completely separate space!
///     rec.log("world/wormhole", &DisconnectedSpace::new(true))?;
///     rec.log("world/wormhole/point", &Points3D::new([(2.0, 2.0, 2.0)]))?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct DisconnectedSpace {
    pub disconnected_space: crate::components::DisconnectedSpace,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.DisconnectedSpace".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.DisconnectedSpaceIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.DisconnectedSpace".into(),
            "rerun.components.DisconnectedSpaceIndicator".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl DisconnectedSpace {
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`DisconnectedSpace`] [`crate::Archetype`]
pub type DisconnectedSpaceIndicator = crate::GenericIndicatorComponent<DisconnectedSpace>;

impl crate::Archetype for DisconnectedSpace {
    type Indicator = DisconnectedSpaceIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.DisconnectedSpace".into()
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
            Some((&self.disconnected_space as &dyn crate::ComponentBatch).into()),
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
                let array =
                    <crate::components::DisconnectedSpace>::try_to_arrow(
                        [&self.disconnected_space],
                    );
                array.map(|array| {
                    let datatype = ::arrow2::datatypes::DataType::Extension(
                        "rerun.components.DisconnectedSpace".into(),
                        Box::new(array.data_type().clone()),
                        None,
                    );
                    (
                        ::arrow2::datatypes::Field::new("disconnected_space", datatype, false),
                        array,
                    )
                })
            })
            .transpose()
            .with_context("rerun.archetypes.DisconnectedSpace#disconnected_space")?
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
        let disconnected_space = {
            let array = arrays_by_name
                .get("disconnected_space")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.DisconnectedSpace#disconnected_space")?;
            <crate::components::DisconnectedSpace>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.DisconnectedSpace#disconnected_space")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.DisconnectedSpace#disconnected_space")?
        };
        Ok(Self { disconnected_space })
    }
}

impl DisconnectedSpace {
    pub fn new(disconnected_space: impl Into<crate::components::DisconnectedSpace>) -> Self {
        Self {
            disconnected_space: disconnected_space.into(),
        }
    }
}
