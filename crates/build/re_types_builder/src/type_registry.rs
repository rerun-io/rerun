//! The bridge between the two halves of the type system: [`Type`] in, [`DataType`] out.

use std::collections::HashMap;

use anyhow::Context as _;

use crate::data_type::{AtomicDataType, DataType, Field, UnionMode};
use crate::{ArrowAttr, Object, ObjectField, Objects, Type};

// --- Registry ---

/// The arrow [`DataType`] of everything a definition declares.
///
/// Keyed by fully-qualified name, both of objects (`rerun.datatypes.Vec3D`) and of their fields
/// (`rerun.components.Position2D#position`), because the backends ask by name from wherever they
/// happen to be.
#[derive(Debug, Default)]
pub struct TypeRegistry {
    registry: HashMap<String, DataType>,
}

impl TypeRegistry {
    /// Computes the Arrow datatype of every object, and of every field of every object.
    pub fn from_objects(objects: &Objects) -> Self {
        let mut this = Self::default();
        for obj in objects.objects.values() {
            this.register(objects, obj);
        }
        this
    }

    /// Retrieves the [`DataType`] associated with the given fully-qualified
    /// name, if any.
    pub fn try_get(&self, fqname: impl AsRef<str>) -> Option<DataType> {
        self.registry.get(fqname.as_ref()).cloned()
    }

    /// Retrieves the [`DataType`] associated with the given fully-qualified
    /// name.
    ///
    /// Panics if missing.
    pub fn get(&self, fqname: impl AsRef<str>) -> DataType {
        let fqname = fqname.as_ref();
        self.try_get(fqname)
            .with_context(|| format!("{fqname:?} not found in Arrow registry"))
            .unwrap()
    }

    // ---

    /// Computes the datatype of `obj`, unless we already have it.
    ///
    /// Objects are registered in whatever order they happen to come in, so an object we depend on
    /// may not be registered yet; we then register it right here, recursively.
    fn register(&mut self, objects: &Objects, obj: &Object) -> DataType {
        if let Some(datatype) = self.try_get(&obj.fqname) {
            return datatype;
        }

        let datatype = self.datatype_from_object(objects, obj);
        self.registry.insert(obj.fqname.clone(), datatype.clone());
        datatype
    }

    fn datatype_from_object(&mut self, objects: &Objects, obj: &Object) -> DataType {
        let is_arrow_transparent = obj.is_arrow_transparent();
        let num_fields = obj.fields.len();

        if is_arrow_transparent {
            assert!(
                obj.is_struct(),
                "{}: arrow-transparent objects must be structs; {:?} is {:?}",
                obj.virtpath,
                obj.fqname,
                obj.class
            );
            assert!(
                num_fields == 1,
                "{}: arrow-transparent structs must have exactly one field, but {:?} has {num_fields}",
                obj.virtpath,
                obj.fqname,
            );

            DataType::Object {
                fqname: obj.fqname.clone(),
                datatype: self.datatype_from_field(objects, &obj.fields[0]).into(),
            }
        } else {
            match obj.class {
                crate::ObjectClass::Struct => {
                    let fields = obj
                        .fields
                        .iter()
                        .map(|obj_field| Field {
                            name: obj_field.name.clone(),
                            data_type: self.datatype_from_field(objects, obj_field),
                            is_nullable: obj_field.is_nullable,
                        })
                        .collect();

                    DataType::Object {
                        fqname: obj.fqname.clone(),
                        datatype: DataType::Struct(fields).into(),
                    }
                }
                crate::ObjectClass::Enum(enum_integer_type) => DataType::Object {
                    fqname: obj.fqname.clone(),
                    datatype: DataType::Atomic(enum_integer_type.to_atomic()).into(),
                },
                crate::ObjectClass::Union => {
                    let union_mode = if obj.is_attr_set(ArrowAttr::SparseUnion) {
                        UnionMode::Sparse
                    } else {
                        UnionMode::Dense
                    };

                    // NOTE: Inject the null markers' field first and foremost! That way it is
                    // guaranteed to be stable and forward-compatible.
                    let null_markers = std::iter::once(Field {
                        name: "_null_markers".into(),
                        data_type: AtomicDataType::Null.into(),
                        // NOTE: The spec doesn't allow a `Null` array to be non-nullable. Not that
                        // we care either way.
                        is_nullable: true,
                    });

                    let fields = obj.fields.iter().map(|field| Field {
                        name: field.name.clone(),
                        data_type: self.datatype_from_field(objects, field),
                        // NOTE: The spec doesn't allow a `Null` array to be non-nullable.
                        // The unit type of an enum field is a `Null`, so this must be nullable.
                        is_nullable: field.typ.is_unit(),
                    });

                    DataType::Object {
                        fqname: obj.fqname.clone(),
                        datatype: DataType::Union(
                            std::iter::chain(null_markers, fields).collect(),
                            union_mode,
                        )
                        .into(),
                    }
                }
            }
        }
    }

    /// Also registers the datatype under the field's fully-qualified name.
    fn datatype_from_field(&mut self, objects: &Objects, field: &ObjectField) -> DataType {
        let datatype = self.datatype_from_type(objects, &field.typ);
        self.registry.insert(field.fqname.clone(), datatype.clone());
        datatype
    }

    fn datatype_from_type(&mut self, objects: &Objects, typ: &Type) -> DataType {
        match typ {
            Type::Atomic(atomic) => DataType::Atomic(*atomic),
            Type::Binary => DataType::Binary,
            Type::Utf8 => DataType::Utf8,
            Type::FixedSizeList { elem_type, length } => {
                DataType::FixedSizeList(self.item_field(objects, elem_type).into(), *length)
            }
            Type::List { elem_type } => DataType::List(self.item_field(objects, elem_type).into()),
            Type::Object { fqname } => self.register(objects, &objects[fqname]),
        }
    }

    /// The unnamed element field of a list or a fixed-size list.
    fn item_field(&mut self, objects: &Objects, elem_type: &Type) -> Field {
        Field {
            name: "item".into(),
            data_type: self.datatype_from_type(objects, elem_type),
            // NOTE: Do _not_ confuse this with the nullability of the field itself!
            // This would be the nullability of the elements of the list itself, which the
            // frontend rejects for now (https://github.com/rerun-io/rerun/issues/2993),
            // so you can be certain this is always false.
            is_nullable: false,
        }
    }
}
