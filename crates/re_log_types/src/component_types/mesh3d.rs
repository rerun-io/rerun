use arrow2::array::{FixedSizeBinaryArray, MutableFixedSizeBinaryArray};
use arrow2::buffer::Buffer;
use arrow2::datatypes::DataType;
use arrow2_convert::arrow_enable_vec_for_type;
use arrow2_convert::deserialize::ArrowDeserialize;
use arrow2_convert::field::ArrowField;
use arrow2_convert::{serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

use super::{FieldError, Vec4D};

// ----------------------------------------------------------------------------

/// A unique id per [`Mesh3D`].
///
/// TODO(emilk): this should be a hash of the mesh (CAS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MeshId(pub uuid::Uuid);

impl nohash_hasher::IsEnabled for MeshId {}

// required for [`nohash_hasher`].
#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for MeshId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl MeshId {
    #[inline]
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl ArrowField for MeshId {
    type Type = Self;

    #[inline]
    fn data_type() -> arrow2::datatypes::DataType {
        arrow2::datatypes::DataType::FixedSizeBinary(16)
    }
}

impl ArrowSerialize for MeshId {
    type MutableArrayType = MutableFixedSizeBinaryArray;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        MutableFixedSizeBinaryArray::new(16)
    }

    #[inline]
    fn arrow_serialize(
        v: &<Self as arrow2_convert::field::ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0.as_bytes()))
    }
}

impl ArrowDeserialize for MeshId {
    type ArrayType = FixedSizeBinaryArray;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v.and_then(|bytes| uuid::Uuid::from_slice(bytes).ok())
            .map(Self)
    }
}

// ----------------------------------------------------------------------------

// TODO(cmc): Let's make both mesh Component types use friendlier types for their inner elements
// (e.g. positions should be a vec of Vec3D, transform should be a Mat4, etc).
// This will also make error checking for invalid user data much nicer.
//
// But first let's do the python example and see how everything starts to take shape...

// TODO(cmc): Let's move all the RefCounting stuff to the top-level.

#[derive(thiserror::Error, Debug)]
pub enum RawMeshError {
    #[error("Positions array length must be divisible by 3 (xyz, xyz, …), got {0}")]
    PositionsNotDivisibleBy3(usize),

    #[error("Indices array length must be divisible by 3 (triangle list), got {0}")]
    IndicesNotDivisibleBy3(usize),

    #[error("No indices were specified, so the number of positions must be divisible by 9 [(xyz xyz xyz), …], got {0}")]
    PositionsAreNotTriangles(usize),

    #[error("Index out of bounds: got index={index} with {num_vertices} vertices")]
    IndexOutOfBounds { index: u32, num_vertices: usize },

    #[error(
        "Positions & normals array must have the same length, \
        got positions={0} vs. normals={1}"
    )]
    MismatchedPositionsNormals(usize, usize),
}

/// A raw "triangle soup" mesh.
///
/// ```
/// # use re_log_types::component_types::RawMesh3D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     RawMesh3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("mesh_id", DataType::FixedSizeBinary(16), false),
///         Field::new("vertex_positions", DataType::List(Box::new(
///             Field::new("item", DataType::Float32, false)),
///         ), false),
///         Field::new("vertex_colors", DataType::List(Box::new(
///             Field::new("item", DataType::UInt32, false)),
///         ), true),
///         Field::new("vertex_normals", DataType::List(Box::new(
///             Field::new("item", DataType::Float32, false)),
///         ), true),
///         Field::new("indices", DataType::List(Box::new(
///             Field::new("item", DataType::UInt32, false)),
///         ), true),
///         Field::new("albedo_factor", DataType::FixedSizeList(
///             Box::new(Field::new("item", DataType::Float32, false)),
///             4
///         ), true),
///     ]),
/// );
/// ```
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize, Clone, Debug, PartialEq)]
pub struct RawMesh3D {
    pub mesh_id: MeshId,

    /// The flattened vertex positions array of this mesh.
    ///
    /// The length of this vector should always be divisible by three (since this is a 3D mesh).
    ///
    /// If no indices are specified, then each triplet of vertex positions are interpreted as a triangle
    /// and the length of this must be divisible by 9.
    pub vertex_positions: Buffer<f32>,

    /// Per-vertex albedo colors.
    /// This is actually an encoded [`super::ColorRGBA`]
    pub vertex_colors: Option<Buffer<u32>>,

    /// Optionally, the flattened normals array for this mesh.
    ///
    /// If specified, this must match the length of `Self::positions`.
    pub vertex_normals: Option<Buffer<f32>>,

    /// Optionally, the flattened indices array for this mesh.
    ///
    /// Meshes are always triangle lists, i.e. the length of this vector should always be
    /// divisible by three.
    pub indices: Option<Buffer<u32>>,

    /// Albedo factor applied to the final color of the mesh.
    ///
    /// `[1.0, 1.0, 1.0, 1.0]` if unspecified.
    pub albedo_factor: Option<Vec4D>,
    //
    // TODO(cmc): We need to support vertex colors and/or texturing, otherwise it's pretty
    // hard to see anything with complex enough meshes (and hovering doesn't really help
    // when everything's white).
    // pub colors: Option<Vec<u8>>,
    // pub texcoords: Option<Vec<f32>>,
}

impl RawMesh3D {
    pub fn sanity_check(&self) -> Result<(), RawMeshError> {
        if self.vertex_positions.len() % 3 != 0 {
            return Err(RawMeshError::PositionsNotDivisibleBy3(
                self.vertex_positions.len(),
            ));
        }

        let num_vertices = self.vertex_positions.len() / 3;

        if let Some(indices) = &self.indices {
            if indices.len() % 3 != 0 {
                return Err(RawMeshError::IndicesNotDivisibleBy3(indices.len()));
            }

            for &index in indices.iter() {
                if num_vertices <= index as usize {
                    return Err(RawMeshError::IndexOutOfBounds {
                        index,
                        num_vertices,
                    });
                }
            }
        } else if self.vertex_positions.len() % 9 != 0 {
            return Err(RawMeshError::PositionsAreNotTriangles(
                self.vertex_positions.len(),
            ));
        }

        if let Some(normals) = &self.vertex_normals {
            if normals.len() != self.vertex_positions.len() {
                return Err(RawMeshError::MismatchedPositionsNormals(
                    self.vertex_positions.len(),
                    normals.len(),
                ));
            }
        }

        Ok(())
    }

    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.vertex_positions.len() / 3
    }

    #[inline]
    pub fn num_triangles(&self) -> usize {
        if let Some(indices) = &self.indices {
            indices.len() / 3
        } else {
            self.num_vertices() / 3
        }
    }
}

// ----------------------------------------------------------------------------

/// Compressed/encoded mesh format
///
/// ```
/// # use re_log_types::component_types::EncodedMesh3D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     EncodedMesh3D::data_type(),
///     DataType::Struct(vec![
///         Field::new("mesh_id", DataType::FixedSizeBinary(16), false),
///         Field::new("format", DataType::Union(vec![
///             Field::new("Gltf", DataType::Boolean, false),
///             Field::new("Glb", DataType::Boolean, false),
///             Field::new("Obj", DataType::Boolean, false),
///         ], None, UnionMode::Dense), false),
///         Field::new("bytes", DataType::Binary, false),
///         Field::new("transform", DataType::FixedSizeList(
///             Box::new(Field::new("item", DataType::Float32, false)),
///             12,
///         ), false),
///     ]),
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedMesh3D {
    pub mesh_id: MeshId,

    pub format: MeshFormat,

    pub bytes: Buffer<u8>,

    /// four columns of an affine transformation matrix
    pub transform: [[f32; 3]; 4],
}

/// Helper struct for converting `EncodedMesh3D` to arrow
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct EncodedMesh3DArrow {
    pub mesh_id: MeshId,

    pub format: MeshFormat,

    pub bytes: Buffer<u8>,

    #[arrow_field(type = "arrow2_convert::field::FixedSizeVec<f32, 12>")]
    pub transform: Vec<f32>,
}

impl From<&EncodedMesh3D> for EncodedMesh3DArrow {
    fn from(v: &EncodedMesh3D) -> Self {
        let EncodedMesh3D {
            mesh_id,
            format,
            bytes,
            transform,
        } = v;
        Self {
            mesh_id: *mesh_id,
            format: *format,
            bytes: bytes.clone(),
            transform: transform.iter().flat_map(|c| c.iter().cloned()).collect(),
        }
    }
}

impl TryFrom<EncodedMesh3DArrow> for EncodedMesh3D {
    type Error = FieldError;

    fn try_from(v: EncodedMesh3DArrow) -> super::Result<Self> {
        let EncodedMesh3DArrow {
            mesh_id,
            format,
            bytes,
            transform,
        } = v;

        Ok(Self {
            mesh_id,
            format,
            bytes,
            transform: [
                transform.as_slice()[0..3].try_into()?,
                transform.as_slice()[3..6].try_into()?,
                transform.as_slice()[6..9].try_into()?,
                transform.as_slice()[9..12].try_into()?,
            ],
        })
    }
}

arrow_enable_vec_for_type!(EncodedMesh3D);

impl ArrowField for EncodedMesh3D {
    type Type = Self;

    #[inline]
    fn data_type() -> DataType {
        <EncodedMesh3DArrow as ArrowField>::data_type()
    }
}

impl ArrowSerialize for EncodedMesh3D {
    type MutableArrayType = <EncodedMesh3DArrow as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        EncodedMesh3DArrow::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        <EncodedMesh3DArrow as ArrowSerialize>::arrow_serialize(&v.into(), array)
    }
}

impl ArrowDeserialize for EncodedMesh3D {
    type ArrayType = <EncodedMesh3DArrow as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        let v = <EncodedMesh3DArrow as ArrowDeserialize>::arrow_deserialize(v);
        v.and_then(|v| v.try_into().ok())
    }
}

// ----------------------------------------------------------------------------

/// The format of a binary mesh file, e.g. GLTF, GLB, OBJ
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum MeshFormat {
    /// [`glTF`](https://en.wikipedia.org/wiki/GlTF).
    Gltf,

    /// Binary [`glTF`](https://en.wikipedia.org/wiki/GlTF).
    Glb,

    /// [Wavefront .obj](https://en.wikipedia.org/wiki/Wavefront_.obj_file).
    Obj,
}

impl std::fmt::Display for MeshFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeshFormat::Gltf => "GLTF".fmt(f),
            MeshFormat::Glb => "GLB".fmt(f),
            MeshFormat::Obj => "OBJ".fmt(f),
        }
    }
}

/// A Generic 3D Mesh.
///
/// Cheaply clonable as it is all refcounted internally.
///
/// ```
/// # use re_log_types::component_types::{Mesh3D, EncodedMesh3D, RawMesh3D};
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     Mesh3D::data_type(),
///     DataType::Union(vec![
///         Field::new("Encoded", EncodedMesh3D::data_type(), false),
///         Field::new("Raw", RawMesh3D::data_type(), false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
pub enum Mesh3D {
    Encoded(EncodedMesh3D),
    Raw(RawMesh3D),
}

impl Component for Mesh3D {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.mesh3d".into()
    }
}

impl Mesh3D {
    #[inline]
    pub fn mesh_id(&self) -> MeshId {
        match self {
            Mesh3D::Encoded(mesh) => mesh.mesh_id,
            Mesh3D::Raw(mesh) => mesh.mesh_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_raw_mesh() -> RawMesh3D {
        let mesh = RawMesh3D {
            mesh_id: MeshId::random(),
            vertex_positions: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 9.0, 10.0].into(),
            vertex_colors: Some(vec![0xff0000ff, 0x00ff00ff, 0x0000ffff].into()),
            indices: Some(vec![0, 1, 2].into()),
            vertex_normals: Some(
                vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 80.0, 90.0, 100.0].into(),
            ),
            albedo_factor: Vec4D([0.5, 0.5, 0.5, 1.0]).into(),
        };
        mesh.sanity_check().unwrap();
        mesh
    }

    #[test]
    fn test_mesh_roundtrip() {
        use arrow2::array::Array;
        use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

        // Encoded
        {
            let mesh_in = vec![Mesh3D::Encoded(EncodedMesh3D {
                mesh_id: MeshId::random(),
                format: MeshFormat::Glb,
                bytes: vec![5, 9, 13, 95, 38, 42, 98, 17].into(),
                transform: [
                    [1.0, 2.0, 3.0],
                    [4.0, 5.0, 6.0],
                    [7.0, 8.0, 9.0],
                    [10.0, 11.0, 12.],
                ],
            })];
            let array: Box<dyn Array> = mesh_in.try_into_arrow().unwrap();
            let mesh_out: Vec<Mesh3D> = TryIntoCollection::try_into_collection(array).unwrap();
            assert_eq!(mesh_in, mesh_out);
        }

        // Raw
        {
            let mesh_in = vec![Mesh3D::Raw(example_raw_mesh())];
            let array: Box<dyn Array> = mesh_in.try_into_arrow().unwrap();
            let mesh_out: Vec<Mesh3D> = TryIntoCollection::try_into_collection(array).unwrap();
            assert_eq!(mesh_in, mesh_out);
        }
    }
}
