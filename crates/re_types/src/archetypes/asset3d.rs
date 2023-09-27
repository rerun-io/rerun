// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/asset3d.fbs".

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

/// A prepacked 3D asset (`.gltf`, `.glb`, `.obj`, etc).
///
/// ## Examples
///
/// Simple 3D asset:
/// ```ignore
/// //! Log a simple 3D asset.
///
/// use rerun::{
///     archetypes::{Asset3D, ViewCoordinates},
///     external::anyhow,
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), anyhow::Error> {
///     let args = std::env::args().collect::<Vec<_>>();
///     let Some(path) = args.get(1) else {
///         anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb]>", args[0]);
///     };
///
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_asset3d_simple").memory()?;
///
///     rec.log_timeless("world", &ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
///     rec.log("world/asset", &Asset3D::from_file(path)?)?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
///
/// 3D asset with out-of-tree transform:
/// ```ignore
/// //! Log a simple 3D asset with an out-of-tree transform which will not affect its children.
///
/// use rerun::{
///     archetypes::{Asset3D, Points3D, ViewCoordinates},
///     components::OutOfTreeTransform3D,
///     datatypes::TranslationRotationScale3D,
///     demo_util::grid,
///     external::{anyhow, glam},
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), anyhow::Error> {
///     let args = std::env::args().collect::<Vec<_>>();
///     let Some(path) = args.get(1) else {
///         anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb]>", args[0]);
///     };
///
///     let (rec, storage) =
///         RecordingStreamBuilder::new("rerun_example_asset3d_out_of_tree").memory()?;
///
///     rec.log_timeless("world", &ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
///
///     rec.set_time_sequence("frame", 0);
///     rec.log("world/asset", &Asset3D::from_file(path)?)?;
///     // Those points will not be affected by their parent's out-of-tree transform!
///     rec.log(
///         "world/asset/points",
///         &Points3D::new(grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10)),
///     )?;
///
///     for i in 1..20 {
///         rec.set_time_sequence("frame", i);
///
///         // Modify the asset's out-of-tree transform: this will not affect its children (i.e. the points)!
///         let translation = TranslationRotationScale3D::translation([0.0, 0.0, i as f32 - 10.0]);
///         rec.log_component_batches(
///             "world/asset",
///             false,
///             [&OutOfTreeTransform3D::from(translation) as _],
///         )?;
///     }
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Asset3D {
    /// The asset's bytes.
    pub data: crate::components::Blob,

    /// The Media Type of the asset.
    ///
    /// For instance:
    /// * `model/gltf-binary`
    /// * `model/obj`
    ///
    /// If omitted, the viewer will try to guess from the data.
    /// If it cannot guess, it won't be able to render the asset.
    pub media_type: Option<crate::components::MediaType>,

    /// An out-of-tree transform.
    ///
    /// Applies a transformation to the asset itself without impacting its children.
    pub transform: Option<crate::components::OutOfTreeTransform3D>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Blob".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Asset3DIndicator".into(),
            "rerun.components.MediaType".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.InstanceKey".into(),
            "rerun.components.OutOfTreeTransform3D".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Blob".into(),
            "rerun.components.Asset3DIndicator".into(),
            "rerun.components.MediaType".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.OutOfTreeTransform3D".into(),
        ]
    });

impl Asset3D {
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`Asset3D`] [`crate::Archetype`]
pub type Asset3DIndicator = crate::GenericIndicatorComponent<Asset3D>;

impl crate::Archetype for Asset3D {
    type Indicator = Asset3DIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.Asset3D".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: Asset3DIndicator = Asset3DIndicator::DEFAULT;
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
    fn num_instances(&self) -> usize {
        1
    }

    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        [
            Some(Self::indicator()),
            Some((&self.data as &dyn crate::ComponentBatch).into()),
            self.media_type
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.transform
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
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
        Ok([
            {
                Some({
                    let array = <crate::components::Blob>::try_to_arrow([&self.data]);
                    array.map(|array| {
                        let datatype = ::arrow2::datatypes::DataType::Extension(
                            "rerun.components.Blob".into(),
                            Box::new(array.data_type().clone()),
                            None,
                        );
                        (
                            ::arrow2::datatypes::Field::new("data", datatype, false),
                            array,
                        )
                    })
                })
                .transpose()
                .with_context("rerun.archetypes.Asset3D#data")?
            },
            {
                self.media_type
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::MediaType>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.MediaType".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("media_type", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.Asset3D#media_type")?
            },
            {
                self.transform
                    .as_ref()
                    .map(|single| {
                        let array =
                            <crate::components::OutOfTreeTransform3D>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.OutOfTreeTransform3D".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("transform", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.Asset3D#transform")?
            },
        ]
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
        let data = {
            let array = arrays_by_name
                .get("data")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Asset3D#data")?;
            <crate::components::Blob>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Asset3D#data")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Asset3D#data")?
        };
        let media_type = if let Some(array) = arrays_by_name.get("media_type") {
            Some({
                <crate::components::MediaType>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Asset3D#media_type")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.Asset3D#media_type")?
            })
        } else {
            None
        };
        let transform = if let Some(array) = arrays_by_name.get("transform") {
            Some({
                <crate::components::OutOfTreeTransform3D>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Asset3D#transform")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.Asset3D#transform")?
            })
        } else {
            None
        };
        Ok(Self {
            data,
            media_type,
            transform,
        })
    }
}

impl Asset3D {
    pub fn new(data: impl Into<crate::components::Blob>) -> Self {
        Self {
            data: data.into(),
            media_type: None,
            transform: None,
        }
    }

    pub fn with_media_type(mut self, media_type: impl Into<crate::components::MediaType>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    pub fn with_transform(
        mut self,
        transform: impl Into<crate::components::OutOfTreeTransform3D>,
    ) -> Self {
        self.transform = Some(transform.into());
        self
    }
}
