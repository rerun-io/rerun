// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/pinhole.fbs".

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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Camera perspective projection (a.k.a. intrinsics).
///
/// ## Examples
///
/// ### Simple pinhole camera
/// ```ignore
/// use ndarray::{Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_pinhole").spawn()?;
///
///     let mut image = Array::<u8, _>::default((3, 3, 3).f());
///     image.map_inplace(|x| *x = rand::random());
///
///     rec.log(
///         "world/image",
///         &rerun::Pinhole::from_focal_length_and_resolution([3., 3.], [3., 3.]),
///     )?;
///     rec.log(
///         "world/image",
///         &rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image)?,
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/1200w.png">
///   <img src="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/full.png" width="640">
/// </picture>
/// </center>
///
/// ### Perspective pinhole camera
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_pinhole_perspective").spawn()?;
///
///     let fov_y = std::f32::consts::FRAC_PI_4;
///     let aspect_ratio = 1.7777778;
///     rec.log(
///         "world/cam",
///         &rerun::Pinhole::from_fov_and_aspect_ratio(fov_y, aspect_ratio)
///             .with_camera_xyz(rerun::components::ViewCoordinates::RUB)
///             .with_image_plane_distance(0.1),
///     )?;
///
///     rec.log(
///         "world/points",
///         &rerun::Points3D::new([(0.0, 0.0, -0.5), (0.1, 0.1, -0.5), (-0.1, -0.1, -0.5)])
///             .with_radii([0.025]),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/1200w.png">
///   <img src="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pinhole {
    /// Camera projection, from image coordinates to view coordinates.
    pub image_from_camera: Option<SerializedComponentBatch>,

    /// Pixel resolution (usually integers) of child image space. Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    ///
    /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
    pub resolution: Option<SerializedComponentBatch>,

    /// Sets the view coordinates for the camera.
    ///
    /// All common values are available as constants on the [`components::ViewCoordinates`][crate::components::ViewCoordinates] class.
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
    pub camera_xyz: Option<SerializedComponentBatch>,

    /// The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
    ///
    /// This is only used for visualization purposes, and does not affect the projection itself.
    pub image_plane_distance: Option<SerializedComponentBatch>,
}

impl Pinhole {
    /// Returns the [`ComponentDescriptor`] for [`Self::image_from_camera`].
    #[inline]
    pub fn descriptor_image_from_camera() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.Pinhole".into()),
            component_name: "rerun.components.PinholeProjection".into(),
            archetype_field_name: Some("image_from_camera".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::resolution`].
    #[inline]
    pub fn descriptor_resolution() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.Pinhole".into()),
            component_name: "rerun.components.Resolution".into(),
            archetype_field_name: Some("resolution".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::camera_xyz`].
    #[inline]
    pub fn descriptor_camera_xyz() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.Pinhole".into()),
            component_name: "rerun.components.ViewCoordinates".into(),
            archetype_field_name: Some("camera_xyz".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::image_plane_distance`].
    #[inline]
    pub fn descriptor_image_plane_distance() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.Pinhole".into()),
            component_name: "rerun.components.ImagePlaneDistance".into(),
            archetype_field_name: Some("image_plane_distance".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.Pinhole".into()),
            component_name: "rerun.components.PinholeIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [Pinhole::descriptor_image_from_camera()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            Pinhole::descriptor_resolution(),
            Pinhole::descriptor_indicator(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            Pinhole::descriptor_camera_xyz(),
            Pinhole::descriptor_image_plane_distance(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            Pinhole::descriptor_image_from_camera(),
            Pinhole::descriptor_resolution(),
            Pinhole::descriptor_indicator(),
            Pinhole::descriptor_camera_xyz(),
            Pinhole::descriptor_image_plane_distance(),
        ]
    });

impl Pinhole {
    /// The total number of components in the archetype: 1 required, 2 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 5usize;
}

/// Indicator component for the [`Pinhole`] [`::re_types_core::Archetype`]
pub type PinholeIndicator = ::re_types_core::GenericIndicatorComponent<Pinhole>;

impl ::re_types_core::Archetype for Pinhole {
    type Indicator = PinholeIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Pinhole".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Pinhole"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: PinholeIndicator = PinholeIndicator::DEFAULT;
        ComponentBatchCowWithDescriptor::new(&INDICATOR as &dyn ::re_types_core::ComponentBatch)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentDescriptor, arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_descr: ::nohash_hasher::IntMap<_, _> = arrow_data.into_iter().collect();
        let image_from_camera = arrays_by_descr
            .get(&Self::descriptor_image_from_camera())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_image_from_camera())
            });
        let resolution = arrays_by_descr
            .get(&Self::descriptor_resolution())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_resolution())
            });
        let camera_xyz = arrays_by_descr
            .get(&Self::descriptor_camera_xyz())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_camera_xyz())
            });
        let image_plane_distance = arrays_by_descr
            .get(&Self::descriptor_image_plane_distance())
            .map(|array| {
                SerializedComponentBatch::new(
                    array.clone(),
                    Self::descriptor_image_plane_distance(),
                )
            });
        Ok(Self {
            image_from_camera,
            resolution,
            camera_xyz,
            image_plane_distance,
        })
    }
}

impl ::re_types_core::AsComponents for Pinhole {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.image_from_camera.clone(),
            self.resolution.clone(),
            self.camera_xyz.clone(),
            self.image_plane_distance.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for Pinhole {}

impl Pinhole {
    /// Create a new `Pinhole`.
    #[inline]
    pub fn new(image_from_camera: impl Into<crate::components::PinholeProjection>) -> Self {
        Self {
            image_from_camera: try_serialize_field(
                Self::descriptor_image_from_camera(),
                [image_from_camera],
            ),
            resolution: None,
            camera_xyz: None,
            image_plane_distance: None,
        }
    }

    /// Update only some specific fields of a `Pinhole`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `Pinhole`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            image_from_camera: Some(SerializedComponentBatch::new(
                crate::components::PinholeProjection::arrow_empty(),
                Self::descriptor_image_from_camera(),
            )),
            resolution: Some(SerializedComponentBatch::new(
                crate::components::Resolution::arrow_empty(),
                Self::descriptor_resolution(),
            )),
            camera_xyz: Some(SerializedComponentBatch::new(
                crate::components::ViewCoordinates::arrow_empty(),
                Self::descriptor_camera_xyz(),
            )),
            image_plane_distance: Some(SerializedComponentBatch::new(
                crate::components::ImagePlaneDistance::arrow_empty(),
                Self::descriptor_image_plane_distance(),
            )),
        }
    }

    /// Partitions the component data into multiple sub-batches.
    ///
    /// Specifically, this transforms the existing [`SerializedComponentBatch`]es data into [`SerializedComponentColumn`]s
    /// instead, via [`SerializedComponentBatch::partitioned`].
    ///
    /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
    ///
    /// The specified `lengths` must sum to the total length of the component batch.
    ///
    /// [`SerializedComponentColumn`]: [::re_types_core::SerializedComponentColumn]
    #[inline]
    pub fn columns<I>(
        self,
        _lengths: I,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>>
    where
        I: IntoIterator<Item = usize> + Clone,
    {
        let columns = [
            self.image_from_camera
                .map(|image_from_camera| image_from_camera.partitioned(_lengths.clone()))
                .transpose()?,
            self.resolution
                .map(|resolution| resolution.partitioned(_lengths.clone()))
                .transpose()?,
            self.camera_xyz
                .map(|camera_xyz| camera_xyz.partitioned(_lengths.clone()))
                .transpose()?,
            self.image_plane_distance
                .map(|image_plane_distance| image_plane_distance.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Helper to partition the component data into unit-length sub-batches.
    ///
    /// This is semantically similar to calling [`Self::columns`] with `std::iter::take(1).repeat(n)`,
    /// where `n` is automatically guessed.
    #[inline]
    pub fn columns_of_unit_batches(
        self,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>> {
        let len_image_from_camera = self.image_from_camera.as_ref().map(|b| b.array.len());
        let len_resolution = self.resolution.as_ref().map(|b| b.array.len());
        let len_camera_xyz = self.camera_xyz.as_ref().map(|b| b.array.len());
        let len_image_plane_distance = self.image_plane_distance.as_ref().map(|b| b.array.len());
        let len = None
            .or(len_image_from_camera)
            .or(len_resolution)
            .or(len_camera_xyz)
            .or(len_image_plane_distance)
            .unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// Camera projection, from image coordinates to view coordinates.
    #[inline]
    pub fn with_image_from_camera(
        mut self,
        image_from_camera: impl Into<crate::components::PinholeProjection>,
    ) -> Self {
        self.image_from_camera =
            try_serialize_field(Self::descriptor_image_from_camera(), [image_from_camera]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::PinholeProjection`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_image_from_camera`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_image_from_camera(
        mut self,
        image_from_camera: impl IntoIterator<Item = impl Into<crate::components::PinholeProjection>>,
    ) -> Self {
        self.image_from_camera =
            try_serialize_field(Self::descriptor_image_from_camera(), image_from_camera);
        self
    }

    /// Pixel resolution (usually integers) of child image space. Width and height.
    ///
    /// Example:
    /// ```text
    /// [1920.0, 1440.0]
    /// ```
    ///
    /// `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
    #[inline]
    pub fn with_resolution(mut self, resolution: impl Into<crate::components::Resolution>) -> Self {
        self.resolution = try_serialize_field(Self::descriptor_resolution(), [resolution]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::Resolution`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_resolution`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_resolution(
        mut self,
        resolution: impl IntoIterator<Item = impl Into<crate::components::Resolution>>,
    ) -> Self {
        self.resolution = try_serialize_field(Self::descriptor_resolution(), resolution);
        self
    }

    /// Sets the view coordinates for the camera.
    ///
    /// All common values are available as constants on the [`components::ViewCoordinates`][crate::components::ViewCoordinates] class.
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
    #[inline]
    pub fn with_camera_xyz(
        mut self,
        camera_xyz: impl Into<crate::components::ViewCoordinates>,
    ) -> Self {
        self.camera_xyz = try_serialize_field(Self::descriptor_camera_xyz(), [camera_xyz]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::ViewCoordinates`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_camera_xyz`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_camera_xyz(
        mut self,
        camera_xyz: impl IntoIterator<Item = impl Into<crate::components::ViewCoordinates>>,
    ) -> Self {
        self.camera_xyz = try_serialize_field(Self::descriptor_camera_xyz(), camera_xyz);
        self
    }

    /// The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
    ///
    /// This is only used for visualization purposes, and does not affect the projection itself.
    #[inline]
    pub fn with_image_plane_distance(
        mut self,
        image_plane_distance: impl Into<crate::components::ImagePlaneDistance>,
    ) -> Self {
        self.image_plane_distance = try_serialize_field(
            Self::descriptor_image_plane_distance(),
            [image_plane_distance],
        );
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::ImagePlaneDistance`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_image_plane_distance`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_image_plane_distance(
        mut self,
        image_plane_distance: impl IntoIterator<Item = impl Into<crate::components::ImagePlaneDistance>>,
    ) -> Self {
        self.image_plane_distance = try_serialize_field(
            Self::descriptor_image_plane_distance(),
            image_plane_distance,
        );
        self
    }
}

impl ::re_byte_size::SizeBytes for Pinhole {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.image_from_camera.heap_size_bytes()
            + self.resolution.heap_size_bytes()
            + self.camera_xyz.heap_size_bytes()
            + self.image_plane_distance.heap_size_bytes()
    }
}
