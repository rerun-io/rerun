//! The Arrow registry keeps track of all type definitions and maps them to Arrow datatypes.

use anyhow::Context as _;
use arrow2::datatypes::{DataType, Field, UnionMode};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{ElementType, Object, ObjectField, Type, ATTR_ARROW_SPARSE_UNION};

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
    pub fn register(&mut self, obj: &mut Object) {
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

    fn arrow_datatype_from_object(&mut self, obj: &mut Object) -> LazyDatatype {
        let is_struct = obj.is_struct();
        let is_enum = obj.is_enum();
        let is_arrow_transparent = obj.is_arrow_transparent();
        let num_fields = obj.fields.len();

        if is_arrow_transparent {
            assert!(
                is_struct,
                "{}: arrow-transparent objects must be structs; {:?} is {:?}",
                obj.virtpath, obj.fqname, obj.class
            );
            assert!(
                num_fields == 1,
                "{}: arrow-transparent structs must have exactly one field, but {:?} has {num_fields}",
                obj.virtpath,
                obj.fqname,
            );
        }

        let datatype = if is_arrow_transparent {
            LazyDatatype::Extension(
                obj.fqname.clone(),
                Box::new(
                    self.arrow_datatype_from_type(obj.fields[0].typ.clone(), &mut obj.fields[0]),
                ),
                None,
            )
        } else if is_struct {
            LazyDatatype::Extension(
                obj.fqname.clone(),
                Box::new(LazyDatatype::Struct(
                    obj.fields
                        .iter_mut()
                        .map(|obj_field| LazyField {
                            name: obj_field.name.clone(),
                            datatype: self
                                .arrow_datatype_from_type(obj_field.typ.clone(), obj_field),
                            is_nullable: obj_field.is_nullable,
                            metadata: Default::default(),
                        })
                        .collect(),
                )),
                None,
            )
        } else if is_enum {
            // TODO(jleibs): The underlying type is encoded in the FBS and could be used
            // here instead if we want non-u8 enums.
            LazyDatatype::Extension(obj.fqname.clone(), Box::new(LazyDatatype::UInt8), None)
        } else {
            let is_sparse = obj.is_attr_set(ATTR_ARROW_SPARSE_UNION);
            let union_mode = if is_sparse {
                arrow2::datatypes::UnionMode::Sparse
            } else {
                arrow2::datatypes::UnionMode::Dense
            };

            // NOTE: Inject the null markers' field first and foremost! That way it is
            // guaranteed to be stable and forward-compatible.
            let fields = std::iter::once(LazyField {
                name: "_null_markers".into(),
                datatype: LazyDatatype::Null,
                // NOTE: The spec doesn't allow a `Null` array to be non-nullable. Not that
                // we care either way.
                is_nullable: true,
                metadata: Default::default(),
            })
            .chain(obj.fields.iter_mut().map(|field| LazyField {
                name: field.name.clone(),
                datatype: self.arrow_datatype_from_type(field.typ.clone(), field),
                // NOTE: The spec doesn't allow a `Null` array to be non-nullable.
                // We map Unit -> Null in enum fields, so this must be nullable.
                is_nullable: field.typ == Type::Unit,
                metadata: Default::default(),
            }))
            .collect();

            LazyDatatype::Extension(
                obj.fqname.clone(),
                Box::new(LazyDatatype::Union(
                    fields,
                    // NOTE: +1 to account for virtual nullability arm
                    Some((0..(obj.fields.len() + 1) as i32).collect()),
                    union_mode,
                )),
                None,
            )
        };

        // NOTE: Arrow-transparent objects by definition don't have a datatype of their own.
        if !is_arrow_transparent {
            obj.datatype = datatype.clone().into();
        }

        datatype
    }

    fn arrow_datatype_from_type(&mut self, typ: Type, field: &mut ObjectField) -> LazyDatatype {
        let datatype = match typ {
            Type::Unit => LazyDatatype::Null,
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
                    // NOTE: Do _not_ confuse this with the nullability of the field itself!
                    // This would be the nullability of the elements of the list itself, which our IDL
                    // literally is unable to express at the moment, so you can be certain this is
                    // always false.
                    is_nullable: false,
                    metadata: Default::default(),
                }),
                length,
            ),
            Type::Vector { elem_type } => LazyDatatype::List(Box::new(LazyField {
                name: "item".into(),
                datatype: self.arrow_datatype_from_element_type(elem_type),
                // NOTE: Do _not_ confuse this with the nullability of the field itself!
                // This would be the nullability of the elements of the list itself, which our IDL
                // literally is unable to express at the moment, so you can be certain this is
                // always false.
                is_nullable: false,
                metadata: Default::default(),
            })),
            Type::Object { fqname } => LazyDatatype::Unresolved { fqname },
        };

        field.datatype = datatype.clone().into();
        self.registry.insert(field.fqname.clone(), datatype.clone());

        datatype
    }

    fn arrow_datatype_from_element_type(&self, typ: ElementType) -> LazyDatatype {
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
            ElementType::Object { fqname } => LazyDatatype::Unresolved { fqname },
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
    Unresolved { fqname: String },
}

impl From<DataType> for LazyDatatype {
    fn from(datatype: DataType) -> Self {
        match datatype {
            DataType::Null => Self::Null,
            DataType::Boolean => Self::Boolean,
            DataType::Int8 => Self::Int8,
            DataType::Int16 => Self::Int16,
            DataType::Int32 => Self::Int32,
            DataType::Int64 => Self::Int64,
            DataType::UInt8 => Self::UInt8,
            DataType::UInt16 => Self::UInt16,
            DataType::UInt32 => Self::UInt32,
            DataType::UInt64 => Self::UInt64,
            DataType::Float16 => Self::Float16,
            DataType::Float32 => Self::Float32,
            DataType::Float64 => Self::Float64,
            DataType::Binary => Self::Binary,
            DataType::FixedSizeBinary(length) => Self::FixedSizeBinary(length),
            DataType::LargeBinary => Self::LargeBinary,
            DataType::Utf8 => Self::Utf8,
            DataType::LargeUtf8 => Self::LargeUtf8,
            DataType::List(field) => Self::List(Box::new((*field).clone().into())),
            DataType::FixedSizeList(field, length) => {
                Self::FixedSizeList(Box::new((*field).clone().into()), length)
            }
            DataType::LargeList(field) => Self::LargeList(Box::new((*field).clone().into())),
            DataType::Struct(fields) => {
                Self::Struct(fields.iter().cloned().map(Into::into).collect())
            }
            DataType::Union(fields, x, mode) => Self::Union(
                fields.iter().cloned().map(Into::into).collect(),
                x.map(|arc| arc.to_vec()),
                mode,
            ),
            DataType::Extension(name, datatype, metadata) => Self::Extension(
                name,
                Box::new((*datatype).clone().into()),
                metadata.map(|arc| arc.to_string()),
            ),
            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

impl LazyDatatype {
    /// Recursively resolves the datatype using the specified `registry`.
    fn resolve(&self, registry: &ArrowRegistry) -> DataType {
        match self {
            Self::Null => DataType::Null,
            Self::Boolean => DataType::Boolean,
            Self::Int8 => DataType::Int8,
            Self::Int16 => DataType::Int16,
            Self::Int32 => DataType::Int32,
            Self::Int64 => DataType::Int64,
            Self::UInt8 => DataType::UInt8,
            Self::UInt16 => DataType::UInt16,
            Self::UInt32 => DataType::UInt32,
            Self::UInt64 => DataType::UInt64,
            Self::Float16 => DataType::Float16,
            Self::Float32 => DataType::Float32,
            Self::Float64 => DataType::Float64,
            Self::Binary => DataType::Binary,
            Self::FixedSizeBinary(length) => DataType::FixedSizeBinary(*length),
            Self::LargeBinary => DataType::LargeBinary,
            Self::Utf8 => DataType::Utf8,
            Self::LargeUtf8 => DataType::LargeUtf8,
            Self::List(field) => DataType::List(Arc::new(field.resolve(registry))),
            Self::FixedSizeList(field, length) => {
                DataType::FixedSizeList(Arc::new(field.resolve(registry)), *length)
            }
            Self::LargeList(field) => DataType::LargeList(Arc::new(field.resolve(registry))),
            Self::Struct(fields) => DataType::Struct(Arc::new(
                fields.iter().map(|field| field.resolve(registry)).collect(),
            )),
            Self::Union(fields, x, mode) => DataType::Union(
                Arc::new(fields.iter().map(|field| field.resolve(registry)).collect()),
                x.as_ref().map(|x| Arc::new(x.clone())),
                *mode,
            ),
            Self::Extension(name, datatype, metadata) => DataType::Extension(
                name.clone(),
                Arc::new(datatype.resolve(registry)),
                metadata.as_ref().map(|s| Arc::new(s.clone())),
            ),
            Self::Unresolved { fqname } => registry.get(fqname),
        }
    }
}
