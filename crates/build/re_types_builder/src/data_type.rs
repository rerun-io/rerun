//! The arrow half of the type system: what a definition's data looks like once serialized.
//!
//! Nothing in here is written by hand. [`TypeRegistry`](crate::TypeRegistry) derives it all from
//! the definition half — [`Type`](crate::Type) — and the backends write each SDK's
//! (de)serializers against it.

use std::sync::Arc;

use strum::Display;

use crate::objects::enum_obj_of;
use crate::{Object, Objects};

/// Whether the arms of a [`DataType::Union`] each get their own slots, or share one set.
///
/// See the arrow docs for what that means for the layout. Ours are dense unless a definition says
/// otherwise with `#[arrow(sparse_union)]`.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum UnionMode {
    /// Dense union
    Dense,

    /// Sparse union
    Sparse,
}

/// A named [`DataType`]: a member of a struct or a union, or the `item` of a list.
///
/// Corresponds to an arrow field.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Field {
    /// Its name
    pub name: String,

    /// Its logical [`DataType`]
    pub data_type: DataType,

    /// Its nullability
    pub is_nullable: bool,
}

impl Field {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_nullable(&self) -> bool {
        self.is_nullable
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

/// A fixed-size scalar: a number, a boolean, or nothing.
///
/// Shared by both halves of the type system — [`DataType::Atomic`] and
/// [`Type::Atomic`](crate::Type::Atomic) — because the two agree exactly on which scalars exist.
/// It is the one place their variants are spelled out.
///
/// [`Self::Null`] doubles as the unit type of an `enum` variant with no payload; see
/// [`Type::UNIT`](crate::Type::UNIT).
#[derive(Debug, Clone, Copy, Display, Hash, PartialEq, Eq)]
pub enum AtomicDataType {
    Null,
    Boolean,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float16,
    Float32,
    Float64,
}

impl AtomicDataType {
    /// Is this the `null` type, i.e. our unit type?
    pub fn is_null(self) -> bool {
        self == Self::Null
    }

    /// Is this type directly backed by a native arrow `Buffer`, i.e. can it be used with
    /// `arrow::ScalarBuffer`?
    ///
    /// That gives zero-copy access to a slice of the data.
    pub fn backed_by_scalar_buffer(self) -> bool {
        !matches!(self, Self::Null | Self::Boolean)
    }
}

/// An arrow datatype, limited to what our definitions can express.
///
/// Every variant but [`Self::Object`] is an arrow datatype as arrow means it, so this is mostly a
/// mirror of `arrow::datatypes::DataType`. `Object` is ours, and is why we do not use arrow's type
/// directly: it remembers which definition a datatype came from, at every level of nesting, which
/// is what lets generated code say `<Vec3D>::arrow_datatype()` instead of spelling out the whole
/// nested struct. Look through it with [`Self::to_logical_type`].
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DataType {
    Atomic(AtomicDataType),

    /// A list of bytes of arbitrary length, generated as arrow's `LargeBinary`.
    Binary,

    /// A string of arbitrary length.
    Utf8,

    List(Arc<Field>),

    FixedSizeList(Arc<Field>, usize),

    Struct(Vec<Field>),

    /// The placement in the list is also its identifier.
    Union(Vec<Field>, UnionMode),

    /// A datatype together with the name of the definition it came from.
    Object {
        /// Its fully-qualified name, e.g. `rerun.datatypes.Vec3D`.
        fqname: String,

        /// What it actually is, e.g. a [`DataType::Struct`].
        datatype: Arc<Self>,
    },
}

impl DataType {
    /// Strips any [`Self::Object`] wrappers, leaving the datatype they name.
    ///
    /// Anything that cares about the *shape* of the data has to go through here first, since an
    /// `Object` can wrap any of the other variants.
    // TODO(emilk) make this type-safe instead, i.e. return a different type.
    pub fn to_logical_type(&self) -> &Self {
        if let Self::Object { datatype, .. } = self {
            datatype.to_logical_type()
        } else {
            self
        }
    }

    /// Can this type be used with `arrow::ScalarBuffer`?
    ///
    /// That gives zero-copy access to a slice of the data.
    pub fn backed_by_scalar_buffer(&self) -> bool {
        match self {
            Self::Atomic(atomic) => atomic.backed_by_scalar_buffer(),
            _ => false,
        }
    }

    /// `Some(Object)` if this is an enum object.
    pub fn enum_obj<'a>(&self, objects: &'a Objects) -> Option<&'a Object> {
        match self {
            Self::Object { fqname, .. } => enum_obj_of(objects, fqname),
            _ => None,
        }
    }
}

impl From<AtomicDataType> for DataType {
    fn from(atomic: AtomicDataType) -> Self {
        Self::Atomic(atomic)
    }
}
