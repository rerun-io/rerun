use arrow2::array::{FixedSizeBinaryArray, MutableFixedSizeBinaryArray};
use arrow2::datatypes::DataType;
use arrow2_convert::arrow_enable_vec_for_type;
use arrow2_convert::deserialize::ArrowDeserialize;
use arrow2_convert::field::ArrowField;
use arrow2_convert::{serialize::ArrowSerialize, ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

use super::FieldError;

// ----------------------------------------------------------------------------

/// A unique id per [`Mesh3D`].
///
/// TODO(emilk): this should be a hash of the mesh (CAS).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MeshId(pub uuid::Uuid);

impl nohash_hasher::IsEnabled for MeshId {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
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

    fn data_type() -> arrow2::datatypes::DataType {
        arrow2::datatypes::DataType::FixedSizeBinary(16)
    }
}

impl ArrowSerialize for MeshId {
    type MutableArrayType = MutableFixedSizeBinaryArray;

    fn new_array() -> Self::MutableArrayType {
        MutableFixedSizeBinaryArray::new(16)
    }

    fn arrow_serialize(
        v: &<Self as arrow2_convert::field::ArrowField>::Type,
        array: &mut Self::MutableArrayType,
    ) -> arrow2::error::Result<()> {
        array.try_push(Some(v.0.as_bytes()))
    }
}

impl ArrowDeserialize for MeshId {
    type ArrayType = FixedSizeBinaryArray;

    fn arrow_deserialize(
        v: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        v.and_then(|bytes| uuid::Uuid::from_slice(bytes).ok())
            .map(Self)
    }
}

// ----------------------------------------------------------------------------

// TODO(#749) Re-enable `RawMesh3D`
// These seem totally unused at the moment and not even supported by the SDK
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RawMesh3D {
    pub mesh_id: MeshId,
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
}

// ----------------------------------------------------------------------------

/// Compressed/encoded mesh format
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EncodedMesh3D {
    pub mesh_id: MeshId,
    pub format: MeshFormat,
    pub bytes: std::sync::Arc<[u8]>,
    /// four columns of an affine transformation matrix
    pub transform: [[f32; 3]; 4],
}

/// Helper struct for converting `EncodedMesh3D` to arrow
#[derive(ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct EncodedMesh3DArrow {
    pub mesh_id: MeshId,
    pub format: MeshFormat,
    pub bytes: Vec<u8>,
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
            bytes: bytes.as_ref().into(),
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
            bytes: bytes.into(),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum MeshFormat {
    Gltf,
    Glb,
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

/// A Generic 3D Mesh
///
/// ```
/// # use re_log_types::component_types::Mesh3D;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field, UnionMode};
/// assert_eq!(
///     Mesh3D::data_type(),
///     DataType::Union(
///         vec![Field::new(
///             "Encoded",
///             DataType::Struct(vec![
///                 Field::new("mesh_id", DataType::FixedSizeBinary(16), false),
///                 Field::new(
///                     "format",
///                     DataType::Union(
///                         vec![
///                             Field::new("Gltf", DataType::Boolean, false),
///                             Field::new("Glb", DataType::Boolean, false),
///                             Field::new("Obj", DataType::Boolean, false)
///                         ],
///                         None,
///                         UnionMode::Dense
///                     ),
///                     false
///                 ),
///                 Field::new("bytes", DataType::Binary, false),
///                 Field::new(
///                     "transform",
///                     DataType::FixedSizeList(
///                         Box::new(Field::new("item", DataType::Float32, false)),
///                         12
///                     ),
///                     false
///                 )
///             ]),
///             false
///         )],
///         None,
///         UnionMode::Dense
///     )
/// );
/// ```
#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(type = "dense")]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Mesh3D {
    Encoded(EncodedMesh3D),
    // TODO(#749) Re-enable `RawMesh3D`
    // Raw(Arc<RawMesh3D>),
}

impl Component for Mesh3D {
    fn name() -> crate::ComponentName {
        "rerun.mesh3d".into()
    }
}

impl Mesh3D {
    pub fn mesh_id(&self) -> MeshId {
        match self {
            Mesh3D::Encoded(mesh) => mesh.mesh_id,
            // TODO(#749) Re-enable `RawMesh3D`
            // Mesh3D::Raw(mesh) => mesh.mesh_id,
        }
    }
}

#[test]
fn test_datatype() {}

#[test]
fn test_mesh_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let mesh_in = vec![Mesh3D::Encoded(EncodedMesh3D {
        mesh_id: MeshId::random(),
        format: MeshFormat::Glb,
        bytes: std::sync::Arc::new([5, 9, 13, 95, 38, 42, 98, 17]),
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
