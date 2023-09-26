// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/pinhole.fbs".

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

/// Camera perspective projection (a.k.a. intrinsics).
///
/// ## Example
///
/// ```ignore
/// //! Log a pinhole and a random image.
///
/// use ndarray::{Array, ShapeBuilder};
/// use rerun::{
///     archetypes::{Image, Pinhole},
///     RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_pinhole").memory()?;
///
///     let mut image = Array::<u8, _>::default((3, 3, 3).f());
///     image.map_inplace(|x| *x = rand::random());
///
///     rec.log(
///         "world/image",
///         &Pinhole::from_focal_length_and_resolution([3., 3.], [3., 3.]),
///     )?;
///     rec.log("world/image", &Image::try_from(image)?)?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Pinhole {
    /// Camera projection, from image coordinates to view coordinates.
    pub image_from_camera: crate::components::PinholeProjection,

    /// Pixel resolution (usually integers) of child image space. Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    ///
    /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
    pub resolution: Option<crate::components::Resolution>,

    /// Sets the view coordinates for the camera.
    ///
    /// All common values are available as constants on the `components.ViewCoordinates` class.
    ///
    /// The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
    /// This means that the camera frustum will point along the positive Z axis of the parent space,
    /// and the cameras "up" direction will be along the negative Y axis of the parent space.
    ///
    /// The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
    /// When logging a depth image under this entity, this is the direction the point cloud will be projected.
    /// With `RDF`, the default forward is +Z.
    ///
    /// The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
    /// This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
    /// With `RDF`, the default is up is -Y.
    ///
    /// The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
    /// This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
    /// With `RDF`, the default right is +x.
    ///
    /// Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).
    ///
    /// NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
    /// and make the pinhole matrix not match up with the coordinate system of the pinhole entity.
    ///
    /// The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
    /// but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
    pub camera_xyz: Option<crate::components::ViewCoordinates>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.PinholeProjection".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.PinholeIndicator".into(),
            "rerun.components.Resolution".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.InstanceKey".into(),
            "rerun.components.ViewCoordinates".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.PinholeProjection".into(),
            "rerun.components.PinholeIndicator".into(),
            "rerun.components.Resolution".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.ViewCoordinates".into(),
        ]
    });

impl Pinhole {
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`Pinhole`] [`crate::Archetype`]
pub type PinholeIndicator = crate::GenericIndicatorComponent<Pinhole>;

impl crate::Archetype for Pinhole {
    type Indicator = PinholeIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.Pinhole".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: PinholeIndicator = PinholeIndicator::DEFAULT;
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
        let image_from_camera = {
            let array = arrays_by_name
                .get("image_from_camera")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Pinhole#image_from_camera")?;
            <crate::components::PinholeProjection>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Pinhole#image_from_camera")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Pinhole#image_from_camera")?
        };
        let resolution = if let Some(array) = arrays_by_name.get("resolution") {
            Some({
                <crate::components::Resolution>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Pinhole#resolution")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.Pinhole#resolution")?
            })
        } else {
            None
        };
        let camera_xyz = if let Some(array) = arrays_by_name.get("camera_xyz") {
            Some({
                <crate::components::ViewCoordinates>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Pinhole#camera_xyz")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.Pinhole#camera_xyz")?
            })
        } else {
            None
        };
        Ok(Self {
            image_from_camera,
            resolution,
            camera_xyz,
        })
    }
}

impl crate::AsComponents for Pinhole {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.image_from_camera as &dyn crate::ComponentBatch).into()),
            self.resolution
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.camera_xyz
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
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

impl Pinhole {
    pub fn new(image_from_camera: impl Into<crate::components::PinholeProjection>) -> Self {
        Self {
            image_from_camera: image_from_camera.into(),
            resolution: None,
            camera_xyz: None,
        }
    }

    pub fn with_resolution(mut self, resolution: impl Into<crate::components::Resolution>) -> Self {
        self.resolution = Some(resolution.into());
        self
    }

    pub fn with_camera_xyz(
        mut self,
        camera_xyz: impl Into<crate::components::ViewCoordinates>,
    ) -> Self {
        self.camera_xyz = Some(camera_xyz.into());
        self
    }
}
