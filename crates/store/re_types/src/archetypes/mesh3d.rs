// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/mesh3d.fbs".

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

/// **Archetype**: A 3D triangle mesh as specified by its per-mesh and per-vertex properties.
///
/// See also [`archetypes::Asset3D`][crate::archetypes::Asset3D].
///
/// If there are multiple [`archetypes::InstancePoses3D`][crate::archetypes::InstancePoses3D] instances logged to the same entity as a mesh,
/// an instance of the mesh will be drawn for each transform.
///
/// ## Examples
///
/// ### Simple indexed 3D mesh
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_mesh3d_indexed").spawn()?;
///
///     rec.log(
///         "triangle",
///         &rerun::Mesh3D::new([[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]])
///             .with_vertex_normals([[0.0, 0.0, 1.0]])
///             .with_vertex_colors([0x0000FFFF, 0x00FF00FF, 0xFF0000FF])
///             .with_triangle_indices([[2, 1, 0]]),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/1200w.png">
///   <img src="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/full.png" width="640">
/// </picture>
/// </center>
///
/// ### 3D mesh with instancing
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_mesh3d_instancing").spawn()?;
///
///     rec.set_time_sequence("frame", 0);
///     rec.log(
///         "shape",
///         &rerun::Mesh3D::new([
///             [1.0, 1.0, 1.0],
///             [-1.0, -1.0, 1.0],
///             [-1.0, 1.0, -1.0],
///             [1.0, -1.0, -1.0],
///         ])
///         .with_triangle_indices([[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]])
///         .with_vertex_colors([0xFF0000FF, 0x00FF00FF, 0x00000FFFF, 0xFFFF00FF]),
///     )?;
///     // This box will not be affected by its parent's instance poses!
///     rec.log(
///         "shape/box",
///         &rerun::Boxes3D::from_half_sizes([[5.0, 5.0, 5.0]]),
///     )?;
///
///     for i in 0..100 {
///         rec.set_time_sequence("frame", i);
///         rec.log(
///             "shape",
///             &rerun::InstancePoses3D::new()
///                 .with_translations([
///                     [2.0, 0.0, 0.0],
///                     [0.0, 2.0, 0.0],
///                     [0.0, -2.0, 0.0],
///                     [-2.0, 0.0, 0.0],
///                 ])
///                 .with_rotation_axis_angles([rerun::RotationAxisAngle::new(
///                     [0.0, 0.0, 1.0],
///                     rerun::Angle::from_degrees(i as f32 * 2.0),
///                 )]),
///         )?;
///     }
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/1200w.png">
///   <img src="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh3D {
    /// The positions of each vertex.
    ///
    /// If no `triangle_indices` are specified, then each triplet of positions is interpreted as a triangle.
    pub vertex_positions: Vec<crate::components::Position3D>,

    /// Optional indices for the triangles that make up the mesh.
    pub triangle_indices: Option<Vec<crate::components::TriangleIndices>>,

    /// An optional normal for each vertex.
    pub vertex_normals: Option<Vec<crate::components::Vector3D>>,

    /// An optional color for each vertex.
    pub vertex_colors: Option<Vec<crate::components::Color>>,

    /// An optional uv texture coordinate for each vertex.
    pub vertex_texcoords: Option<Vec<crate::components::Texcoord2D>>,

    /// A color multiplier applied to the whole mesh.
    pub albedo_factor: Option<crate::components::AlbedoFactor>,

    /// Optional albedo texture.
    ///
    /// Used with the [`components::Texcoord2D`][crate::components::Texcoord2D] of the mesh.
    ///
    /// Currently supports only sRGB(A) textures, ignoring alpha.
    /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
    pub albedo_texture: Option<crate::components::TensorData>,

    /// Optional class Ids for the vertices.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    pub class_ids: Option<Vec<crate::components::ClassId>>,
}

impl ::re_types_core::SizeBytes for Mesh3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.vertex_positions.heap_size_bytes()
            + self.triangle_indices.heap_size_bytes()
            + self.vertex_normals.heap_size_bytes()
            + self.vertex_colors.heap_size_bytes()
            + self.vertex_texcoords.heap_size_bytes()
            + self.albedo_factor.heap_size_bytes()
            + self.albedo_texture.heap_size_bytes()
            + self.class_ids.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::components::Position3D>>::is_pod()
            && <Option<Vec<crate::components::TriangleIndices>>>::is_pod()
            && <Option<Vec<crate::components::Vector3D>>>::is_pod()
            && <Option<Vec<crate::components::Color>>>::is_pod()
            && <Option<Vec<crate::components::Texcoord2D>>>::is_pod()
            && <Option<crate::components::AlbedoFactor>>::is_pod()
            && <Option<crate::components::TensorData>>::is_pod()
            && <Option<Vec<crate::components::ClassId>>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Position3D".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TriangleIndices".into(),
            "rerun.components.Vector3D".into(),
            "rerun.components.Mesh3DIndicator".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.Texcoord2D".into(),
            "rerun.components.AlbedoFactor".into(),
            "rerun.components.TensorData".into(),
            "rerun.components.ClassId".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 9usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Position3D".into(),
            "rerun.components.TriangleIndices".into(),
            "rerun.components.Vector3D".into(),
            "rerun.components.Mesh3DIndicator".into(),
            "rerun.components.Color".into(),
            "rerun.components.Texcoord2D".into(),
            "rerun.components.AlbedoFactor".into(),
            "rerun.components.TensorData".into(),
            "rerun.components.ClassId".into(),
        ]
    });

impl Mesh3D {
    /// The total number of components in the archetype: 1 required, 3 recommended, 5 optional
    pub const NUM_COMPONENTS: usize = 9usize;
}

/// Indicator component for the [`Mesh3D`] [`::re_types_core::Archetype`]
pub type Mesh3DIndicator = ::re_types_core::GenericIndicatorComponent<Mesh3D>;

impl ::re_types_core::Archetype for Mesh3D {
    type Indicator = Mesh3DIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Mesh3D".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Mesh 3D"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: Mesh3DIndicator = Mesh3DIndicator::DEFAULT;
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
        let vertex_positions = {
            let array = arrays_by_name
                .get("rerun.components.Position3D")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.Mesh3D#vertex_positions")?;
            <crate::components::Position3D>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Mesh3D#vertex_positions")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.Mesh3D#vertex_positions")?
        };
        let triangle_indices =
            if let Some(array) = arrays_by_name.get("rerun.components.TriangleIndices") {
                Some({
                    <crate::components::TriangleIndices>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.Mesh3D#triangle_indices")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.Mesh3D#triangle_indices")?
                })
            } else {
                None
            };
        let vertex_normals = if let Some(array) = arrays_by_name.get("rerun.components.Vector3D") {
            Some({
                <crate::components::Vector3D>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Mesh3D#vertex_normals")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Mesh3D#vertex_normals")?
            })
        } else {
            None
        };
        let vertex_colors = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Mesh3D#vertex_colors")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Mesh3D#vertex_colors")?
            })
        } else {
            None
        };
        let vertex_texcoords =
            if let Some(array) = arrays_by_name.get("rerun.components.Texcoord2D") {
                Some({
                    <crate::components::Texcoord2D>::from_arrow_opt(&**array)
                        .with_context("rerun.archetypes.Mesh3D#vertex_texcoords")?
                        .into_iter()
                        .map(|v| v.ok_or_else(DeserializationError::missing_data))
                        .collect::<DeserializationResult<Vec<_>>>()
                        .with_context("rerun.archetypes.Mesh3D#vertex_texcoords")?
                })
            } else {
                None
            };
        let albedo_factor = if let Some(array) = arrays_by_name.get("rerun.components.AlbedoFactor")
        {
            <crate::components::AlbedoFactor>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Mesh3D#albedo_factor")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let albedo_texture = if let Some(array) = arrays_by_name.get("rerun.components.TensorData")
        {
            <crate::components::TensorData>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Mesh3D#albedo_texture")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let class_ids = if let Some(array) = arrays_by_name.get("rerun.components.ClassId") {
            Some({
                <crate::components::ClassId>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.Mesh3D#class_ids")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.Mesh3D#class_ids")?
            })
        } else {
            None
        };
        Ok(Self {
            vertex_positions,
            triangle_indices,
            vertex_normals,
            vertex_colors,
            vertex_texcoords,
            albedo_factor,
            albedo_texture,
            class_ids,
        })
    }
}

impl ::re_types_core::AsComponents for Mesh3D {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.vertex_positions as &dyn ComponentBatch).into()),
            self.triangle_indices
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.vertex_normals
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.vertex_colors
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.vertex_texcoords
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
            self.albedo_factor
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.albedo_texture
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

impl Mesh3D {
    /// Create a new `Mesh3D`.
    #[inline]
    pub fn new(
        vertex_positions: impl IntoIterator<Item = impl Into<crate::components::Position3D>>,
    ) -> Self {
        Self {
            vertex_positions: vertex_positions.into_iter().map(Into::into).collect(),
            triangle_indices: None,
            vertex_normals: None,
            vertex_colors: None,
            vertex_texcoords: None,
            albedo_factor: None,
            albedo_texture: None,
            class_ids: None,
        }
    }

    /// Optional indices for the triangles that make up the mesh.
    #[inline]
    pub fn with_triangle_indices(
        mut self,
        triangle_indices: impl IntoIterator<Item = impl Into<crate::components::TriangleIndices>>,
    ) -> Self {
        self.triangle_indices = Some(triangle_indices.into_iter().map(Into::into).collect());
        self
    }

    /// An optional normal for each vertex.
    #[inline]
    pub fn with_vertex_normals(
        mut self,
        vertex_normals: impl IntoIterator<Item = impl Into<crate::components::Vector3D>>,
    ) -> Self {
        self.vertex_normals = Some(vertex_normals.into_iter().map(Into::into).collect());
        self
    }

    /// An optional color for each vertex.
    #[inline]
    pub fn with_vertex_colors(
        mut self,
        vertex_colors: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.vertex_colors = Some(vertex_colors.into_iter().map(Into::into).collect());
        self
    }

    /// An optional uv texture coordinate for each vertex.
    #[inline]
    pub fn with_vertex_texcoords(
        mut self,
        vertex_texcoords: impl IntoIterator<Item = impl Into<crate::components::Texcoord2D>>,
    ) -> Self {
        self.vertex_texcoords = Some(vertex_texcoords.into_iter().map(Into::into).collect());
        self
    }

    /// A color multiplier applied to the whole mesh.
    #[inline]
    pub fn with_albedo_factor(
        mut self,
        albedo_factor: impl Into<crate::components::AlbedoFactor>,
    ) -> Self {
        self.albedo_factor = Some(albedo_factor.into());
        self
    }

    /// Optional albedo texture.
    ///
    /// Used with the [`components::Texcoord2D`][crate::components::Texcoord2D] of the mesh.
    ///
    /// Currently supports only sRGB(A) textures, ignoring alpha.
    /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
    #[inline]
    pub fn with_albedo_texture(
        mut self,
        albedo_texture: impl Into<crate::components::TensorData>,
    ) -> Self {
        self.albedo_texture = Some(albedo_texture.into());
        self
    }

    /// Optional class Ids for the vertices.
    ///
    /// The [`components::ClassId`][crate::components::ClassId] provides colors and labels if not specified explicitly.
    #[inline]
    pub fn with_class_ids(
        mut self,
        class_ids: impl IntoIterator<Item = impl Into<crate::components::ClassId>>,
    ) -> Self {
        self.class_ids = Some(class_ids.into_iter().map(Into::into).collect());
        self
    }
}
