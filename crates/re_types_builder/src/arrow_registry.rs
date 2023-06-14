//! The Arrow registry keeps track of all type definitions and maps them to Arrow datatypes.

use anyhow::Context as _;
use arrow2::datatypes::{DataType, Field, UnionMode};
use std::collections::{BTreeMap, HashMap};

use crate::{ElementType, Object, Type, ATTR_ARROW_SPARSE_UNION, ATTR_ARROW_TRANSPARENT};

// --- Registry ---

/// Computes and maintains a registry of [`arrow2::datatypes::DataType`]s for specified flatbuffers
/// definitions.
#[derive(Debug, Default)]
pub struct ArrowRegistry {
    registry: HashMap<String, LazyDatatype>,
}

impl ArrowRegistry {
    /// Computes the Arrow datatype for the specified object and stores it in the registry, to be
    /// resolved later on.
    pub fn register(&mut self, obj: &Object) {
        let (fqname, datatype) = (obj.fqname.clone(), self.arrow_datatype_from_object(obj));
        self.registry.insert(fqname, datatype);
    }

    /// Retrieves the [`arrow2::datatypes::DataType`] associated with the given fully-qualified
    /// name, if any.
    ///
    /// This does type resolution just-in-time.
    pub fn try_get(&self, fqname: impl AsRef<str>) -> Option<DataType> {
        self.registry
            .get(fqname.as_ref())
            .map(|dt| dt.resolve(self))
    }

    /// Retrieves the [`arrow2::datatypes::DataType`] associated with the given fully-qualified
    /// name.
    ///
    /// Panics if missing.
    ///
    /// This does type resolution just-in-time.
    pub fn get(&self, fqname: impl AsRef<str>) -> DataType {
        let fqname = fqname.as_ref();
        self.try_get(fqname)
            .with_context(|| format!("{fqname:?} not found in Arrow registry"))
            .unwrap()
    }

    // ---

    fn arrow_datatype_from_object(&self, obj: &Object) -> LazyDatatype {
        let is_struct = obj.is_struct();
        let is_transparent = obj.try_get_attr::<String>(ATTR_ARROW_TRANSPARENT).is_some();
        let num_fields = obj.fields.len();

        assert!(
            !is_transparent || (is_struct && num_fields == 1),
            "cannot have a transparent arrow object with any number of fields but 1: {:?} has {num_fields}",
            obj.fqname,
        );

        if is_transparent {
            self.arrow_datatype_from_type(&obj.fields[0].typ)
        } else if is_struct {
            LazyDatatype::Extension(
                obj.fqname.clone(),
                Box::new(LazyDatatype::Struct(
                    obj.fields
                        .iter()
                        .map(|field| LazyField {
                            name: field.name.clone(),
                            datatype: self.arrow_datatype_from_type(&field.typ),
                            is_nullable: field.required,
                            metadata: Default::default(),
                        })
                        .collect(),
                )),
                None,
            )
        } else {
            let is_sparse = obj
                .try_get_attr::<String>(ATTR_ARROW_SPARSE_UNION)
                .is_some();
            LazyDatatype::Extension(
                obj.fqname.clone(),
                Box::new(LazyDatatype::Union(
                    obj.fields
                        .iter()
                        .map(|field| LazyField {
                            name: field.name.clone(),
                            datatype: self.arrow_datatype_from_type(&field.typ),
                            is_nullable: false,
                            metadata: Default::default(),
                        })
                        .collect(),
                    None,
                    if is_sparse {
                        arrow2::datatypes::UnionMode::Sparse
                    } else {
                        arrow2::datatypes::UnionMode::Dense
                    },
                )),
                None,
            )
        }
    }

    fn arrow_datatype_from_type(&self, typ: &Type) -> LazyDatatype {
        match typ {
            Type::UInt8 => LazyDatatype::UInt8,
            Type::UInt16 => LazyDatatype::UInt16,
            Type::UInt32 => LazyDatatype::UInt32,
            Type::UInt64 => LazyDatatype::UInt64,
            Type::Int8 => LazyDatatype::Int8,
            Type::Int16 => LazyDatatype::Int16,
            Type::Int32 => LazyDatatype::Int32,
            Type::Int64 => LazyDatatype::Int64,
            Type::Bool => LazyDatatype::Boolean,
            Type::Float16 => LazyDatatype::Float16,
            Type::Float32 => LazyDatatype::Float32,
            Type::Float64 => LazyDatatype::Float64,
            Type::String => LazyDatatype::Utf8,
            Type::Array { elem_type, length } => LazyDatatype::FixedSizeList(
                Box::new(LazyField {
                    name: "item".into(),
                    datatype: self.arrow_datatype_from_element_type(elem_type),
                    is_nullable: false,
                    metadata: Default::default(),
                }),
                *length,
            ),
            Type::Vector { elem_type } => LazyDatatype::List(Box::new(LazyField {
                name: "item".into(),
                datatype: self.arrow_datatype_from_element_type(elem_type),
                is_nullable: false,
                metadata: Default::default(),
            })),
            Type::Object(fqname) => LazyDatatype::Unresolved(fqname.clone()),
        }
    }

    fn arrow_datatype_from_element_type(&self, typ: &ElementType) -> LazyDatatype {
        _ = self;
        match typ {
            ElementType::UInt8 => LazyDatatype::UInt8,
            ElementType::UInt16 => LazyDatatype::UInt16,
            ElementType::UInt32 => LazyDatatype::UInt32,
            ElementType::UInt64 => LazyDatatype::UInt64,
            ElementType::Int8 => LazyDatatype::Int8,
            ElementType::Int16 => LazyDatatype::Int16,
            ElementType::Int32 => LazyDatatype::Int32,
            ElementType::Int64 => LazyDatatype::Int64,
            ElementType::Bool => LazyDatatype::Boolean,
            ElementType::Float16 => LazyDatatype::Float16,
            ElementType::Float32 => LazyDatatype::Float32,
            ElementType::Float64 => LazyDatatype::Float64,
            ElementType::String => LazyDatatype::Utf8,
            ElementType::Object(fqname) => LazyDatatype::Unresolved(fqname.clone()),
        }
    }
}

// --- Field ---

/// A yet-to-be-resolved [`arrow2::datatypes::Field`].
///
/// Type resolution is a two-pass process as we first need to register all existing types before we
/// can denormalize their definitions into their parents.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct LazyField {
    /// Its name
    pub name: String,

    /// Its logical [`DataType`]
    pub datatype: LazyDatatype,

    /// Its nullability
    pub is_nullable: bool,

    /// Additional custom (opaque) metadata.
    pub metadata: BTreeMap<String, String>,
}

impl From<Field> for LazyField {
    fn from(field: Field) -> Self {
        let Field {
            name,
            data_type,
            is_nullable,
            metadata,
        } = field;

        Self {
            name,
            datatype: data_type.into(),
            is_nullable,
            metadata,
        }
    }
}

impl LazyField {
    /// Recursively resolves the field using the specified `registry`.
    fn resolve(&self, registry: &ArrowRegistry) -> Field {
        Field {
            name: self.name.clone(),
            data_type: self.datatype.resolve(registry),
            is_nullable: self.is_nullable,
            metadata: self.metadata.clone(),
        }
    }
}

// --- Datatype ---

/// A yet-to-be-resolved [`arrow2::datatypes::DataType`].
///
/// Type resolution is a two-pass process as we first need to register all existing types before we
/// can denormalize their definitions into their parents.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum LazyDatatype {
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
    Binary,
    FixedSizeBinary(usize),
    LargeBinary,
    Utf8,
    LargeUtf8,
    List(Box<LazyField>),
    FixedSizeList(Box<LazyField>, usize),
    LargeList(Box<LazyField>),
    Struct(Vec<LazyField>),
    Union(Vec<LazyField>, Option<Vec<i32>>, UnionMode),
    Extension(String, Box<LazyDatatype>, Option<String>),
    Unresolved(String), // fqname
}

impl From<DataType> for LazyDatatype {
    fn from(datatype: DataType) -> Self {
        match datatype {
            DataType::Null => LazyDatatype::Null,
            DataType::Boolean => LazyDatatype::Boolean,
            DataType::Int8 => LazyDatatype::Int8,
            DataType::Int16 => LazyDatatype::Int16,
            DataType::Int32 => LazyDatatype::Int32,
            DataType::Int64 => LazyDatatype::Int64,
            DataType::UInt8 => LazyDatatype::UInt8,
            DataType::UInt16 => LazyDatatype::UInt16,
            DataType::UInt32 => LazyDatatype::UInt32,
            DataType::UInt64 => LazyDatatype::UInt64,
            DataType::Float16 => LazyDatatype::Float16,
            DataType::Float32 => LazyDatatype::Float32,
            DataType::Float64 => LazyDatatype::Float64,
            DataType::Binary => LazyDatatype::Binary,
            DataType::FixedSizeBinary(length) => LazyDatatype::FixedSizeBinary(length),
            DataType::LargeBinary => LazyDatatype::LargeBinary,
            DataType::Utf8 => LazyDatatype::Utf8,
            DataType::LargeUtf8 => LazyDatatype::LargeUtf8,
            DataType::List(field) => LazyDatatype::List(Box::new((*field).into())),
            DataType::FixedSizeList(field, length) => {
                LazyDatatype::FixedSizeList(Box::new((*field).into()), length)
            }
            DataType::LargeList(field) => LazyDatatype::LargeList(Box::new((*field).into())),
            DataType::Struct(fields) => {
                LazyDatatype::Struct(fields.into_iter().map(Into::into).collect())
            }
            DataType::Union(fields, x, mode) => {
                LazyDatatype::Union(fields.into_iter().map(Into::into).collect(), x, mode)
            }
            DataType::Extension(name, datatype, metadata) => {
                LazyDatatype::Extension(name, Box::new((*datatype).into()), metadata)
            }
            _ => unimplemented!("{datatype:#?}"), // NOLINT
        }
    }
}

impl LazyDatatype {
    /// Recursively resolves the datatype using the specified `registry`.
    fn resolve(&self, registry: &ArrowRegistry) -> DataType {
        match self {
            LazyDatatype::Null => DataType::Null,
            LazyDatatype::Boolean => DataType::Boolean,
            LazyDatatype::Int8 => DataType::Int8,
            LazyDatatype::Int16 => DataType::Int16,
            LazyDatatype::Int32 => DataType::Int32,
            LazyDatatype::Int64 => DataType::Int64,
            LazyDatatype::UInt8 => DataType::UInt8,
            LazyDatatype::UInt16 => DataType::UInt16,
            LazyDatatype::UInt32 => DataType::UInt32,
            LazyDatatype::UInt64 => DataType::UInt64,
            LazyDatatype::Float16 => DataType::Float16,
            LazyDatatype::Float32 => DataType::Float32,
            LazyDatatype::Float64 => DataType::Float64,
            LazyDatatype::Binary => DataType::Binary,
            LazyDatatype::FixedSizeBinary(length) => DataType::FixedSizeBinary(*length),
            LazyDatatype::LargeBinary => DataType::LargeBinary,
            LazyDatatype::Utf8 => DataType::Utf8,
            LazyDatatype::LargeUtf8 => DataType::LargeUtf8,
            LazyDatatype::List(field) => DataType::List(Box::new(field.resolve(registry))),
            LazyDatatype::FixedSizeList(field, length) => {
                DataType::FixedSizeList(Box::new(field.resolve(registry)), *length)
            }
            LazyDatatype::LargeList(field) => {
                DataType::LargeList(Box::new(field.resolve(registry)))
            }
            LazyDatatype::Struct(fields) => {
                DataType::Struct(fields.iter().map(|field| field.resolve(registry)).collect())
            }
            LazyDatatype::Union(fields, x, mode) => DataType::Union(
                fields.iter().map(|field| field.resolve(registry)).collect(),
                x.clone(),
                *mode,
            ),
            LazyDatatype::Extension(name, datatype, metadata) => DataType::Extension(
                name.clone(),
                Box::new(datatype.resolve(registry)),
                metadata.clone(),
            ),
            LazyDatatype::Unresolved(fqname) => registry.get(fqname),
        }
    }
}
