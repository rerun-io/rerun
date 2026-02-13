//! The Arrow registry keeps track of all type definitions and maps them to Arrow datatypes.

use std::collections::HashMap;

use anyhow::Context as _;

use crate::data_type::{AtomicDataType, DataType, LazyDatatype, LazyField, UnionMode};
use crate::objects::EnumIntegerType;
use crate::{ATTR_ARROW_SPARSE_UNION, ElementType, Object, ObjectField, Type};

// --- Registry ---

/// Computes and maintains a registry of [`DataType`]s for specified flatbuffers
/// definitions.
#[derive(Debug, Default)]
pub struct TypeRegistry {
    registry: HashMap<String, LazyDatatype>,
}

impl TypeRegistry {
    /// Computes the Arrow datatype for the specified object and stores it in the registry, to be
    /// resolved later on.
    pub fn register(&mut self, obj: &mut Object) {
        let (fqname, datatype) = (obj.fqname.clone(), self.arrow_datatype_from_object(obj));
        self.registry.insert(fqname, datatype);
    }

    /// Retrieves the [`DataType`] associated with the given fully-qualified
    /// name, if any.
    ///
    /// This does type resolution just-in-time.
    pub fn try_get(&self, fqname: impl AsRef<str>) -> Option<DataType> {
        self.registry
            .get(fqname.as_ref())
            .map(|dt| dt.resolve(self))
    }

    /// Retrieves the [`DataType`] associated with the given fully-qualified
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
            LazyDatatype::Object {
                fqname: obj.fqname.clone(),
                datatype: self
                    .arrow_datatype_from_type(obj.fields[0].typ.clone(), &mut obj.fields[0])
                    .into(),
            }
        } else if is_struct {
            LazyDatatype::Object {
                fqname: obj.fqname.clone(),
                datatype: LazyDatatype::Struct(
                    obj.fields
                        .iter_mut()
                        .map(|obj_field| LazyField {
                            name: obj_field.name.clone(),
                            data_type: self
                                .arrow_datatype_from_type(obj_field.typ.clone(), obj_field),
                            is_nullable: obj_field.is_nullable,
                            metadata: Default::default(),
                        })
                        .collect(),
                )
                .into(),
            }
        } else if let Some(enum_type) = obj.enum_integer_type() {
            let datatype = match enum_type {
                EnumIntegerType::U8 => AtomicDataType::UInt8,
                EnumIntegerType::U16 => AtomicDataType::UInt16,
                EnumIntegerType::U32 => AtomicDataType::UInt32,
                EnumIntegerType::U64 => AtomicDataType::UInt64,
            };
            LazyDatatype::Object {
                fqname: obj.fqname.clone(),
                datatype: LazyDatatype::Atomic(datatype).into(),
            }
        } else {
            let is_sparse = obj.is_attr_set(ATTR_ARROW_SPARSE_UNION);
            let union_mode = if is_sparse {
                UnionMode::Sparse
            } else {
                UnionMode::Dense
            };

            // NOTE: Inject the null markers' field first and foremost! That way it is
            // guaranteed to be stable and forward-compatible.
            let fields = std::iter::once(LazyField {
                name: "_null_markers".into(),
                data_type: AtomicDataType::Null.into(),
                // NOTE: The spec doesn't allow a `Null` array to be non-nullable. Not that
                // we care either way.
                is_nullable: true,
                metadata: Default::default(),
            })
            .chain(obj.fields.iter_mut().map(|field| LazyField {
                name: field.name.clone(),
                data_type: self.arrow_datatype_from_type(field.typ.clone(), field),
                // NOTE: The spec doesn't allow a `Null` array to be non-nullable.
                // We map Unit -> Null in enum fields, so this must be nullable.
                is_nullable: field.typ == Type::Unit,
                metadata: Default::default(),
            }))
            .collect();

            LazyDatatype::Object {
                fqname: obj.fqname.clone(),
                datatype: LazyDatatype::Union(fields, union_mode).into(),
            }
        };

        // NOTE: Arrow-transparent objects by definition don't have a datatype of their own.
        if !is_arrow_transparent {
            obj.datatype = datatype.clone().into();
        }

        datatype
    }

    fn arrow_datatype_from_type(&mut self, typ: Type, field: &mut ObjectField) -> LazyDatatype {
        let datatype = match typ {
            Type::Unit => LazyDatatype::Atomic(AtomicDataType::Null),
            Type::UInt8 => LazyDatatype::Atomic(AtomicDataType::UInt8),
            Type::UInt16 => LazyDatatype::Atomic(AtomicDataType::UInt16),
            Type::UInt32 => LazyDatatype::Atomic(AtomicDataType::UInt32),
            Type::UInt64 => LazyDatatype::Atomic(AtomicDataType::UInt64),
            Type::Int8 => LazyDatatype::Atomic(AtomicDataType::Int8),
            Type::Int16 => LazyDatatype::Atomic(AtomicDataType::Int16),
            Type::Int32 => LazyDatatype::Atomic(AtomicDataType::Int32),
            Type::Int64 => LazyDatatype::Atomic(AtomicDataType::Int64),
            Type::Bool => LazyDatatype::Atomic(AtomicDataType::Boolean),
            Type::Float16 => LazyDatatype::Atomic(AtomicDataType::Float16),
            Type::Float32 => LazyDatatype::Atomic(AtomicDataType::Float32),
            Type::Float64 => LazyDatatype::Atomic(AtomicDataType::Float64),
            Type::Binary => LazyDatatype::Binary,
            Type::String => LazyDatatype::Utf8,
            Type::Array { elem_type, length } => LazyDatatype::FixedSizeList(
                LazyField {
                    name: "item".into(),
                    data_type: self.arrow_datatype_from_element_type(elem_type),
                    // NOTE: Do _not_ confuse this with the nullability of the field itself!
                    // This would be the nullability of the elements of the list itself, which our IDL
                    // literally is unable to express at the moment, so you can be certain this is
                    // always false.
                    is_nullable: false,
                    metadata: Default::default(),
                }
                .into(),
                length,
            ),
            Type::Vector { elem_type } => LazyDatatype::List(
                LazyField {
                    name: "item".into(),
                    data_type: self.arrow_datatype_from_element_type(elem_type),
                    // NOTE: Do _not_ confuse this with the nullability of the field itself!
                    // This would be the nullability of the elements of the list itself, which our IDL
                    // literally is unable to express at the moment, so you can be certain this is
                    // always false.
                    is_nullable: false,
                    metadata: Default::default(),
                }
                .into(),
            ),
            Type::Object { fqname } => LazyDatatype::Unresolved { fqname },
        };

        field.datatype = datatype.clone().into();
        self.registry.insert(field.fqname.clone(), datatype.clone());

        datatype
    }

    fn arrow_datatype_from_element_type(&self, typ: ElementType) -> LazyDatatype {
        _ = self;
        match typ {
            ElementType::UInt8 => LazyDatatype::Atomic(AtomicDataType::UInt8),
            ElementType::UInt16 => LazyDatatype::Atomic(AtomicDataType::UInt16),
            ElementType::UInt32 => LazyDatatype::Atomic(AtomicDataType::UInt32),
            ElementType::UInt64 => LazyDatatype::Atomic(AtomicDataType::UInt64),
            ElementType::Int8 => LazyDatatype::Atomic(AtomicDataType::Int8),
            ElementType::Int16 => LazyDatatype::Atomic(AtomicDataType::Int16),
            ElementType::Int32 => LazyDatatype::Atomic(AtomicDataType::Int32),
            ElementType::Int64 => LazyDatatype::Atomic(AtomicDataType::Int64),
            ElementType::Bool => LazyDatatype::Atomic(AtomicDataType::Boolean),
            ElementType::Float16 => LazyDatatype::Atomic(AtomicDataType::Float16),
            ElementType::Float32 => LazyDatatype::Atomic(AtomicDataType::Float32),
            ElementType::Float64 => LazyDatatype::Atomic(AtomicDataType::Float64),
            ElementType::Binary => LazyDatatype::Binary,
            ElementType::String => LazyDatatype::Utf8,
            ElementType::Object { fqname } => LazyDatatype::Unresolved { fqname },
        }
    }
}
