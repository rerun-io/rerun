//! This is a limited subset of arrow datatypes.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::TypeRegistry;

/// Mode of [`DataType::Union`]
///
/// See arrow docs for explanations.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum UnionMode {
    /// Dense union
    Dense,

    /// Sparse union
    Sparse,
}

/// A named datatype, e.g. for a struct or a union.
///
/// Corresponds to an arrow field.
pub type Field = GenericField<DataType>;

/// A yet-to-be-resolved [`Field`].
///
/// Type resolution is a two-pass process as we first need to register all existing types before we
/// can denormalize their definitions into their parents.
pub type LazyField = GenericField<LazyDatatype>;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GenericField<DT> {
    /// Its name
    pub name: String,

    /// Its logical [`DataType`]
    pub data_type: DT,

    /// Its nullability
    pub is_nullable: bool,

    /// Additional custom (opaque) metadata.
    pub metadata: BTreeMap<String, String>,
}

impl<DT> GenericField<DT> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_nullable(&self) -> bool {
        self.is_nullable
    }

    pub fn data_type(&self) -> &DT {
        &self.data_type
    }
}

impl LazyField {
    pub fn resolve(&self, registry: &TypeRegistry) -> Field {
        Field {
            name: self.name.clone(),
            data_type: self.data_type.resolve(registry),
            is_nullable: self.is_nullable,
            metadata: self.metadata.clone(),
        }
    }
}

/// Simple fixed-size types
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
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

impl std::fmt::Display for AtomicDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => "Null".fmt(f),
            Self::Boolean => "Boolean".fmt(f),
            Self::Int8 => "Int8".fmt(f),
            Self::Int16 => "Int16".fmt(f),
            Self::Int32 => "Int32".fmt(f),
            Self::Int64 => "Int64".fmt(f),
            Self::UInt8 => "UInt8".fmt(f),
            Self::UInt16 => "UInt16".fmt(f),
            Self::UInt32 => "UInt32".fmt(f),
            Self::UInt64 => "UInt64".fmt(f),
            Self::Float16 => "Float16".fmt(f),
            Self::Float32 => "Float32".fmt(f),
            Self::Float64 => "Float64".fmt(f),
        }
    }
}

/// The datatypes we support.
///
/// Maps directly to arrow datatypes.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DataType {
    Atomic(AtomicDataType),

    // 32-bit or 64-bit
    Binary,

    Utf8,

    List(Arc<Field>),

    FixedSizeList(Arc<Field>, usize),

    Struct(Vec<Field>),

    /// The placement in the list is also its identifier.
    Union(Vec<Field>, UnionMode),

    /// A named type.
    Object {
        //// It's fully qualified name
        fqname: String,

        /// It's type (e.g. a [`DataType::Struct`].
        datatype: Arc<Self>,
    },
}

impl DataType {
    /// Resolved [`Self::Object`] to its concrete type.
    // TODO(emilk) make this type-safe instead, i.e. return a different type.
    pub fn to_logical_type(&self) -> &Self {
        if let Self::Object { datatype, .. } = self {
            datatype.to_logical_type()
        } else {
            self
        }
    }
}

/// Like [`DataType`], but with an extra [`Self::Unresolved`] variant
/// which need to be resolved to a concrete type.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum LazyDatatype {
    Atomic(AtomicDataType),

    /// A list of bytes of arbitrary length.
    ///
    /// 32-bit or 64-bit
    Binary,

    /// Utf8
    Utf8,

    /// Elements are non-nullable
    List(Arc<LazyField>),

    /// Elements are non-nullable
    FixedSizeList(Arc<LazyField>, usize),

    Struct(Vec<GenericField<Self>>),

    /// The placement in the list is also its identifier.
    Union(Vec<GenericField<Self>>, UnionMode),

    Object {
        fqname: String,
        datatype: Arc<Self>,
    },

    Unresolved {
        fqname: String,
    },
}

impl From<AtomicDataType> for LazyDatatype {
    fn from(atomic: AtomicDataType) -> Self {
        Self::Atomic(atomic)
    }
}

impl LazyDatatype {
    /// Recursively resolves the datatype using the specified `registry`.
    pub fn resolve(&self, registry: &TypeRegistry) -> DataType {
        match self {
            Self::Atomic(atomic) => DataType::Atomic(*atomic),
            Self::Binary => DataType::Binary,
            Self::Utf8 => DataType::Utf8,
            Self::List(data_type) => DataType::List(data_type.resolve(registry).into()),
            Self::FixedSizeList(datatype, length) => {
                DataType::FixedSizeList(datatype.resolve(registry).into(), *length)
            }
            Self::Struct(fields) => {
                DataType::Struct(fields.iter().map(|field| field.resolve(registry)).collect())
            }
            Self::Union(fields, mode) => DataType::Union(
                fields.iter().map(|field| field.resolve(registry)).collect(),
                *mode,
            ),
            Self::Object { fqname, datatype } => DataType::Object {
                fqname: fqname.clone(),
                datatype: Arc::new(datatype.resolve(registry)),
            },
            Self::Unresolved { fqname } => registry.get(fqname),
        }
    }
}
