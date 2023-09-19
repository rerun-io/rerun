use arrow2::buffer::Buffer;
use arrow2::datatypes::DataType;
use arrow2_convert::arrow_enable_vec_for_type;
use arrow2_convert::deserialize::ArrowDeserialize;
use arrow2_convert::field::ArrowField;
use arrow2_convert::{serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize};

use super::FieldError;

// ----------------------------------------------------------------------------

/// Compressed/encoded mesh format
///
/// ```
/// # use re_components::EncodedMesh3D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     EncodedMesh3D::data_type(),
///     DataType::Struct(vec![
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
    pub format: MeshFormat,

    pub bytes: Buffer<u8>,

    /// four columns of an affine transformation matrix
    pub transform: [[f32; 3]; 4],
}

/// Helper struct for converting `EncodedMesh3D` to arrow
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct EncodedMesh3DArrow {
    pub format: MeshFormat,

    pub bytes: Buffer<u8>,

    #[arrow_field(type = "arrow2_convert::field::FixedSizeVec<f32, 12>")]
    pub transform: Vec<f32>,
}

impl From<&EncodedMesh3D> for EncodedMesh3DArrow {
    fn from(v: &EncodedMesh3D) -> Self {
        let EncodedMesh3D {
            format,
            bytes,
            transform,
        } = v;
        Self {
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
            format,
            bytes,
            transform,
        } = v;

        Ok(Self {
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
/// # use re_components::{Mesh3D, EncodedMesh3D, RawMesh3D};
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     Mesh3D::data_type(),
///     DataType::Union(vec![
///         Field::new("Encoded", EncodedMesh3D::data_type(), false),
///     ], None, UnionMode::Dense),
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
pub enum Mesh3D {
    Encoded(EncodedMesh3D),
}

impl re_log_types::LegacyComponent for Mesh3D {
    #[inline]
    fn legacy_name() -> re_log_types::ComponentName {
        "rerun.mesh3d".into()
    }
}

re_log_types::component_legacy_shim!(Mesh3D);
