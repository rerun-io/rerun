// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/asset3d.fbs".

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

/// **Archetype**: A prepacked 3D asset (`.gltf`, `.glb`, `.obj`, etc.).
///
/// See also [`Mesh3D`][crate::archetypes::Mesh3D].
///
/// ## Example
///
/// ### Simple 3D asset
/// ```ignore
/// use rerun::external::anyhow;
///
/// fn main() -> anyhow::Result<()> {
///     let args = std::env::args().collect::<Vec<_>>();
///     let Some(path) = args.get(1) else {
///         anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb|obj]>", args[0]);
///     };
///
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_asset3d_simple").memory()?;
///
///     rec.log_timeless("world", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
///     rec.log("world/asset", &rerun::Asset3D::from_file(path)?)?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/1200w.png">
///   <img src="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct Asset3D {
    /// The asset's bytes.
    pub blob: crate::components::Blob,

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `model/gltf-binary`
    /// * `model/obj` (.mtl material files are not supported yet, references are silently ignored)
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    pub media_type: Option<crate::components::MediaType>,

    /// An out-of-tree transform.
    ///
    /// Applies a transformation to the asset itself without impacting its children.
    pub transform: Option<crate::components::OutOfTreeTransform3D>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Blob".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Asset3DIndicator".into(),
            "rerun.components.MediaType".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.InstanceKey".into(),
            "rerun.components.OutOfTreeTransform3D".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 5usize]> =
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

/// Indicator component for the [`Asset3D`] [`::re_types_core::Archetype`]
pub type Asset3DIndicator = ::re_types_core::GenericIndicatorComponent<Asset3D>;

impl ::re_types_core::Archetype for Asset3D {
    type Indicator = Asset3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Asset3D".into()
    }

    #[inline]
    fn indicator() -> ::re_types_core::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: Asset3DIndicator = Asset3DIndicator::DEFAULT;
        ::re_types_core::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> ::re_types_core::DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let blob = {
            let array = arrays_by_name
                .get("rerun.components.Blob")
                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Asset3D#blob")?;
            <crate::components::Blob>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Asset3D#blob")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Asset3D#blob")?
        };
        let media_type = if let Some(array) = arrays_by_name.get("rerun.components.MediaType") {
            Some({
                <crate::components::MediaType>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Asset3D#media_type")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(::re_types_core::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.Asset3D#media_type")?
            })
        } else {
            None
        };
        let transform =
            if let Some(array) = arrays_by_name.get("rerun.components.OutOfTreeTransform3D") {
                Some({
                    <crate::components::OutOfTreeTransform3D>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.Asset3D#transform")?
                        .into_iter()
                        .next()
                        .flatten()
                        .ok_or_else(::re_types_core::DeserializationError::missing_data)
                        .with_context("rerun.archetypes.Asset3D#transform")?
                })
            } else {
                None
            };
        Ok(Self {
            blob,
            media_type,
            transform,
        })
    }
}

impl ::re_types_core::AsComponents for Asset3D {
    fn as_component_batches(&self) -> Vec<::re_types_core::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.blob as &dyn ::re_types_core::ComponentBatch).into()),
            self.media_type
                .as_ref()
                .map(|comp| (comp as &dyn ::re_types_core::ComponentBatch).into()),
            self.transform
                .as_ref()
                .map(|comp| (comp as &dyn ::re_types_core::ComponentBatch).into()),
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

impl Asset3D {
    pub fn new(blob: impl Into<crate::components::Blob>) -> Self {
        Self {
            blob: blob.into(),
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
