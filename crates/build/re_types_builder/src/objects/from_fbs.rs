//! The Flatbuffers frontend: turns `.bfbs` reflection data into the IDL-agnostic [`Objects`] IR.
//!
//! Everything that knows about Flatbuffers lives here.
//! The rest of [`crate::objects`] is a plain intermediate representation with no notion of where
//! it came from.

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools as _;

use crate::{
    ATTR_RERUN_OVERRIDE_TYPE, ATTR_RERUN_STATE, Docs, ElementType, FbsBaseType, FbsEnum,
    FbsEnumVal, FbsField, FbsKeyValue, FbsObject, FbsSchema, FbsType, Object, ObjectClass,
    ObjectField, ObjectKind, Objects, Reporter, Type, root_as_schema,
};

use super::{Attributes, EnumIntegerType, State, is_testing_fqname};

// ---

const BUILTIN_UNIT_TYPE_FQNAME: &str = "rerun.builtins.UnitType";

impl Objects {
    /// Runs the semantic pass on a serialized flatbuffers schema.
    ///
    /// The buffer must be a serialized [`FbsSchema`] (i.e. `.bfbs` data).
    pub fn from_buf(
        reporter: &Reporter,
        include_dir_path: impl AsRef<Utf8Path>,
        buf: &[u8],
    ) -> Self {
        let schema = root_as_schema(buf).unwrap();
        Self::from_raw_schema(reporter, include_dir_path, &schema)
    }

    /// Runs the semantic pass on a deserialized flatbuffers [`FbsSchema`].
    pub fn from_raw_schema(
        reporter: &Reporter,
        include_dir_path: impl AsRef<Utf8Path>,
        schema: &FbsSchema<'_>,
    ) -> Self {
        let mut resolved_objs = std::collections::BTreeMap::new();
        let mut resolved_enums = std::collections::BTreeMap::new();

        let enums = schema.enums().iter().collect::<Vec<_>>();
        let objs = schema.objects().iter().collect::<Vec<_>>();

        let include_dir_path = include_dir_path.as_ref();

        // resolve enums
        for enm in schema.enums() {
            let resolved_enum =
                object_from_raw_enum(reporter, include_dir_path, &enums, &objs, &enm);
            resolved_enums.insert(resolved_enum.fqname.clone(), resolved_enum);
        }

        // resolve objects
        for obj in schema.objects() {
            if obj.name() == BUILTIN_UNIT_TYPE_FQNAME {
                continue;
            }

            let resolved_obj =
                object_from_raw_object(reporter, include_dir_path, &enums, &objs, &obj);
            resolved_objs.insert(resolved_obj.fqname.clone(), resolved_obj);
        }

        let mut this = Self {
            objects: std::iter::chain(resolved_enums, resolved_objs).collect(),
        };

        this.resolve_and_validate(reporter);

        this
    }
}

/// Resolves a raw [`FbsObject`] into a higher-level representation that can be easily
/// interpreted and manipulated.
fn object_from_raw_object(
    reporter: &Reporter,
    include_dir_path: impl AsRef<Utf8Path>,
    enums: &[FbsEnum<'_>],
    objs: &[FbsObject<'_>],
    obj: &FbsObject<'_>,
) -> Object {
    let include_dir_path = include_dir_path.as_ref();

    let fqname = obj.name().to_owned();
    let (pkg_name, name) = fqname.rsplit_once('.').map_or_else(
        || panic!("Missing '.' separator in fqname: {fqname:?} - Did you forget to put it in a `namespace`?"),
        |(pkg_name, name)| (pkg_name.to_owned(), name.to_owned()),
    );

    let virtpath = obj
        .declaration_file()
        .map(ToOwned::to_owned)
        .with_context(|| format!("no declaration_file found for {fqname}"))
        .unwrap();
    assert!(virtpath.ends_with(".fbs"), "Bad virtpath: {virtpath:?}");

    let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);
    assert!(
        filepath.to_string().ends_with(".fbs"),
        "Bad filepath: {filepath:?}"
    );

    let docs = docs_from_raw_docs(reporter, &virtpath, obj.name(), obj.documentation());
    let attrs = attributes_from_raw_attrs(obj.attributes());
    let kind = ObjectKind::from_pkg_name(&pkg_name, &attrs);

    let scope = attrs
        .get_string(crate::ATTR_RERUN_SCOPE)
        .or_else(|| (kind == ObjectKind::View).then(|| "blueprint".to_owned()));

    let state = if attrs.has(ATTR_RERUN_STATE) {
        State::from_attrs(&attrs).unwrap_or_else(|err| {
            reporter.error(&virtpath, &fqname, &err);
            State::Stable
        })
    } else if is_testing_fqname(&fqname) {
        State::Stable
    } else if scope == Some("blueprint".to_owned()) {
        State::Unstable // All blueprint APIs are considered unstable unless otherwise specified
    } else {
        match kind {
            ObjectKind::Datatype | ObjectKind::Component => {
                if false {
                    // TODO(#9427): make ATTR_RERUN_STATE attribute mandatory
                    reporter.warn(
                        &virtpath,
                        &fqname,
                        format!("Missing attribute '{ATTR_RERUN_STATE}'"),
                    );
                }
                State::Stable
            }
            ObjectKind::Archetype => {
                reporter.error(
                    &virtpath,
                    &fqname,
                    format!("Missing attribute '{ATTR_RERUN_STATE}'"),
                );
                State::Stable
            }
            ObjectKind::View => State::Unstable,
        }
    };

    let fields: Vec<_> = {
        let mut fields: Vec<_> = obj
            .fields()
            .iter()
            // NOTE: These are intermediate fields used by flatbuffers internals, we don't care.
            .filter(|field| field.type_().base_type() != FbsBaseType::UType)
            .filter(|field| field.type_().element() != FbsBaseType::UType)
            .map(|field| {
                object_field_from_raw_object_field(
                    reporter,
                    include_dir_path,
                    enums,
                    objs,
                    obj,
                    &field,
                )
            })
            .collect();

        // The fields of a struct are reported in arbitrary order by flatbuffers,
        // so we use the `order` attribute to sort them:
        fields.sort_by_key(|field| field.order);

        // Make sure no two fields have the same order:
        for (a, b) in fields.iter().tuple_windows() {
            assert!(
                a.order != b.order,
                "{name:?}: Fields {:?} and {:?} have the same order",
                a.name,
                b.name
            );
        }

        fields
    };

    if kind == ObjectKind::Component {
        assert!(
            fields.len() == 1,
            "components must have exactly 1 field, but {fqname} has {}",
            fields.len()
        );
    }

    Object {
        virtpath,
        filepath,
        fqname,
        pkg_name,
        name,
        docs,
        kind,
        state,
        attrs,
        fields,
        class: ObjectClass::Struct,
        datatype: None,
    }
}

/// Resolves a raw [`FbsEnum`] into a higher-level representation that can be easily
/// interpreted and manipulated.
fn object_from_raw_enum(
    reporter: &Reporter,
    include_dir_path: impl AsRef<Utf8Path>,
    enums: &[FbsEnum<'_>],
    objs: &[FbsObject<'_>],
    enm: &FbsEnum<'_>,
) -> Object {
    let include_dir_path = include_dir_path.as_ref();

    let fqname = enm.name().to_owned();
    let (pkg_name, name) = fqname.rsplit_once('.').map_or_else(
        || panic!("Missing '.' separator in fqname: {fqname:?} - Did you forget to put it in a `namespace`?"),
        |(pkg_name, name)| (pkg_name.to_owned(), name.to_owned()),
    );

    let virtpath = enm
        .declaration_file()
        .map(ToOwned::to_owned)
        .with_context(|| format!("no declaration_file found for {fqname}"))
        .unwrap();
    let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);

    let docs = docs_from_raw_docs(reporter, &virtpath, enm.name(), enm.documentation());
    let attrs = attributes_from_raw_attrs(enm.attributes());
    let kind = ObjectKind::from_pkg_name(&pkg_name, &attrs);
    let state = if attrs.has(ATTR_RERUN_STATE) {
        State::from_attrs(&attrs).unwrap_or_else(|err| {
            reporter.error(
                &virtpath,
                &fqname,
                format!("Failed to parse `{ATTR_RERUN_STATE}`: {err}"),
            );
            State::Stable
        })
    } else {
        State::Stable
    };

    let class = match enm.underlying_type().base_type() {
        FbsBaseType::UByte => ObjectClass::Enum(EnumIntegerType::U8),
        FbsBaseType::UShort => ObjectClass::Enum(EnumIntegerType::U16),
        FbsBaseType::UInt => ObjectClass::Enum(EnumIntegerType::U32),
        FbsBaseType::ULong => ObjectClass::Enum(EnumIntegerType::U64),
        _ => ObjectClass::Union,
    };

    let mut fields: Vec<_> = enm
        .values()
        .iter()
        .filter(|val| {
            // NOTE: `BaseType::None` is only used by internal flatbuffers fields, we don't care.
            class.is_enum()
                || val
                    .union_type()
                    .as_ref()
                    .is_some_and(|utype| utype.base_type() != FbsBaseType::None)
        })
        .map(|val| {
            object_field_from_raw_enum_value(reporter, include_dir_path, enums, objs, enm, &val)
        })
        .collect();

    if class.is_enum() {
        // We want to reserve the value of 0 in all of our enums as an Invalid type variant.
        //
        // The reasoning behind this is twofold:
        // - 0 is a very common accidental value to end up with if processing an incorrectly constructed buffer.
        // - The way the .fbs compiler works, declaring an enum as a member of a struct field either requires the
        //   enum to have a 0 value, or every struct to specify it's contextual default for that enum. This way the
        //   fbs schema definitions are always valid.
        //
        // However, we then remove this field out of our generated types. This means we don't actually have to deal with
        // invalid arms in our enums. Instead we get a deserialization error if someone accidentally uses the invalid 0
        // value in an arrow payload.
        assert!(
            !fields.is_empty(),
            "enums must have at least one variant, but {fqname} has none",
        );

        assert!(
            fields[0].name == "Invalid" && fields[0].enum_or_union_variant_value == Some(0),
            "enums must start with 'Invalid' variant with value 0, but {fqname} starts with {} = {:?}",
            fields[0].name,
            fields[0].enum_or_union_variant_value,
        );

        // Now remove the invalid variant so that it doesn't make it into our native enum definitions.
        fields.remove(0);
    }

    Object {
        virtpath,
        filepath,
        fqname,
        pkg_name,
        name,
        docs,
        kind,
        state,
        attrs,
        fields,
        class,
        datatype: None,
    }
}

fn object_field_from_raw_object_field(
    reporter: &Reporter,
    include_dir_path: impl AsRef<Utf8Path>,
    enums: &[FbsEnum<'_>],
    objs: &[FbsObject<'_>],
    obj: &FbsObject<'_>,
    field: &FbsField<'_>,
) -> ObjectField {
    let fqname = format!("{}#{}", obj.name(), field.name());
    let (pkg_name, name) = fqname.rsplit_once('#').map_or_else(
        || (String::new(), fqname.clone()),
        |(pkg_name, name)| (pkg_name.to_owned(), name.to_owned()),
    );

    let virtpath = obj
        .declaration_file()
        .map(ToOwned::to_owned)
        .with_context(|| format!("no declaration_file found for {fqname}"))
        .unwrap();
    let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);

    if field.required() {
        reporter.error(&virtpath, &fqname, "required fields should not be used");
    }

    let docs = docs_from_raw_docs(reporter, &virtpath, field.name(), field.documentation());

    let attrs = attributes_from_raw_attrs(field.attributes());
    let state = if attrs.has(ATTR_RERUN_STATE) {
        State::from_attrs(&attrs).unwrap_or_else(|err| {
            reporter.error(
                &virtpath,
                &fqname,
                format!("Failed to parse `{ATTR_RERUN_STATE}`: {err}"),
            );
            State::Stable
        })
    } else {
        State::Stable
    };

    let typ = type_from_raw_type(&virtpath, enums, objs, field.type_(), &attrs, &fqname);
    let order = attrs.get::<u32>(&fqname, crate::ATTR_ORDER);

    let is_nullable = attrs.has(crate::ATTR_NULLABLE) || typ == Type::Unit; // null type is always nullable

    if field.deprecated() {
        reporter.warn(
            &virtpath,
            &fqname,
            format!(
                "Use {} attribute for deprecation instead",
                crate::ATTR_RERUN_STATE
            ),
        );
    }

    ObjectField {
        virtpath,
        filepath,
        fqname,
        pkg_name,
        name,
        enum_or_union_variant_value: None,
        docs,
        state,
        typ,
        attrs,
        order,
        is_nullable,
        datatype: None,
    }
}

fn object_field_from_raw_enum_value(
    reporter: &Reporter,
    include_dir_path: impl AsRef<Utf8Path>,
    enums: &[FbsEnum<'_>],
    objs: &[FbsObject<'_>],
    enm: &FbsEnum<'_>,
    val: &FbsEnumVal<'_>,
) -> ObjectField {
    let fqname = format!("{}#{}", enm.name(), val.name());
    let (pkg_name, name) = fqname.rsplit_once('#').map_or_else(
        || (String::new(), fqname.clone()),
        |(pkg_name, name)| (pkg_name.to_owned(), name.to_owned()),
    );

    let virtpath = enm
        .declaration_file()
        .map(ToOwned::to_owned)
        .with_context(|| format!("no declaration_file found for {fqname}"))
        .unwrap();
    let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);

    let docs = docs_from_raw_docs(reporter, &virtpath, val.name(), val.documentation());

    let attrs = attributes_from_raw_attrs(val.attributes());
    let state = if attrs.has(ATTR_RERUN_STATE) {
        State::from_attrs(&attrs).unwrap_or_else(|err| {
            reporter.error(
                &virtpath,
                &fqname,
                format!("Failed to parse `{ATTR_RERUN_STATE}`: {err}"),
            );
            State::Stable
        })
    } else {
        State::Stable
    };

    // NOTE: Unwrapping is safe, we never resolve enums without union types.
    let field_type = val.union_type().unwrap();
    let typ = type_from_raw_type(&virtpath, enums, objs, field_type, &attrs, &fqname);

    let is_nullable = if field_type.base_type() == FbsBaseType::Obj && typ == Type::Unit {
        // Builtin unit type for unions is not nullable.
        false
    } else {
        attrs.has(crate::ATTR_NULLABLE) || typ == Type::Unit // null type is always nullable
    };

    if attrs.has(crate::ATTR_ORDER) {
        reporter.warn(
            &virtpath,
            &fqname,
            "There is no need for an `order` attribute on enum/union variants",
        );
    }

    ObjectField {
        virtpath,
        filepath,
        fqname,
        pkg_name,
        name,
        enum_or_union_variant_value: Some(val.value() as u64),
        state,
        docs,
        typ,
        attrs,
        order: 0, // not needed for enums
        is_nullable,
        datatype: None,
    }
}

fn type_from_raw_type(
    virtpath: &str,
    enums: &[FbsEnum<'_>],
    objs: &[FbsObject<'_>],
    field_type: FbsType<'_>,
    attrs: &Attributes,
    fqname: &str,
) -> Type {
    let typ = field_type.base_type();

    if let Some(type_override) = attrs.try_get::<String>(fqname, ATTR_RERUN_OVERRIDE_TYPE) {
        match type_override.as_str() {
            "binary" => {
                if typ == FbsBaseType::Vector && field_type.element() == FbsBaseType::UByte {
                    return Type::Binary;
                } else {
                    panic!("{fqname}: 'binary' can only be used on '[ubyte]', got {typ:?}")
                }
            }
            "float16" => {
                if matches!(typ, FbsBaseType::Array | FbsBaseType::Vector) {
                    // Array of float16 handled later
                } else if typ == FbsBaseType::UShort {
                    return Type::Float16;
                } else {
                    panic!(
                        "{fqname}: 'float16' can only be used on 'ushort' or `[ushort]`, got {typ:?}"
                    )
                }
            }
            _ => {
                panic!("{fqname}: Unknown {ATTR_RERUN_OVERRIDE_TYPE:?}: {type_override:?}");
            }
        }
    }

    if let Some(enum_fqname) = try_get_enum_fqname(enums, field_type, typ, virtpath) {
        return Type::Object {
            fqname: enum_fqname,
        };
    }

    match typ {
        FbsBaseType::None => Type::Unit, // Enum variant

        FbsBaseType::Bool => Type::Bool,
        FbsBaseType::Byte => Type::Int8,
        FbsBaseType::UByte => Type::UInt8,
        FbsBaseType::Short => Type::Int16,
        FbsBaseType::UShort => Type::UInt16,
        FbsBaseType::Int => Type::Int32,
        FbsBaseType::UInt => Type::UInt32,
        FbsBaseType::Long => Type::Int64,
        FbsBaseType::ULong => Type::UInt64,
        FbsBaseType::Float => Type::Float32,
        FbsBaseType::Double => Type::Float64,
        FbsBaseType::String => Type::String,
        FbsBaseType::Obj => {
            let obj = &objs[field_type.index() as usize];
            if obj.name() == BUILTIN_UNIT_TYPE_FQNAME {
                Type::Unit
            } else {
                Type::Object {
                    fqname: obj.name().to_owned(),
                }
            }
        }
        FbsBaseType::Union => {
            let union = &enums[field_type.index() as usize];
            Type::Object {
                fqname: union.name().to_owned(),
            }
        }
        FbsBaseType::Array => Type::Array {
            elem_type: element_type_from_raw_base_type(
                enums,
                objs,
                field_type,
                field_type.element(),
                attrs,
                virtpath,
            ),
            length: field_type.fixed_length() as usize,
        },
        FbsBaseType::Vector => Type::Vector {
            elem_type: element_type_from_raw_base_type(
                enums,
                objs,
                field_type,
                field_type.element(),
                attrs,
                virtpath,
            ),
        },
        FbsBaseType::UType | FbsBaseType::Vector64 => {
            unimplemented!("FbsBaseType::{typ:#?}")
        }

        // NOTE: `FbsBaseType` isn't actually an enum, it's just a bunch of constants…
        _ => unreachable!("{typ:#?}"),
    }
}

fn element_type_from_raw_base_type(
    enums: &[FbsEnum<'_>],
    objs: &[FbsObject<'_>],
    outer_type: FbsType<'_>,
    inner_type: FbsBaseType,
    attrs: &Attributes,
    virtpath: &str,
) -> ElementType {
    if let Some(enum_fqname) = try_get_enum_fqname(enums, outer_type, inner_type, virtpath) {
        return ElementType::Object {
            fqname: enum_fqname,
        };
    }

    // TODO(jleibs): Clean up fqname plumbing
    let fqname = "???";
    if let Some(type_override) = attrs.try_get::<String>(fqname, ATTR_RERUN_OVERRIDE_TYPE) {
        match (inner_type, type_override.as_str()) {
            (FbsBaseType::UShort, "float16") => {
                return ElementType::Float16;
            }
            _ => unreachable!(
                "UShort -> float16 is the only permitted type override. Not {inner_type:#?}->{type_override}"
            ),
        }
    }

    match inner_type {
        FbsBaseType::Bool => ElementType::Bool,
        FbsBaseType::Byte => ElementType::Int8,
        FbsBaseType::UByte => ElementType::UInt8,
        FbsBaseType::Short => ElementType::Int16,
        FbsBaseType::UShort => ElementType::UInt16,
        FbsBaseType::Int => ElementType::Int32,
        FbsBaseType::UInt => ElementType::UInt32,
        FbsBaseType::Long => ElementType::Int64,
        FbsBaseType::ULong => ElementType::UInt64,
        FbsBaseType::Float => ElementType::Float32,
        FbsBaseType::Double => ElementType::Float64,
        FbsBaseType::String => ElementType::String,
        FbsBaseType::Obj => {
            let obj = &objs[outer_type.index() as usize];
            ElementType::Object {
                fqname: obj.name().to_owned(),
            }
        }
        FbsBaseType::Union => {
            let enm = &enums[outer_type.index() as usize];
            ElementType::Object {
                fqname: enm.name().to_owned(),
            }
        }
        FbsBaseType::None
        | FbsBaseType::UType
        | FbsBaseType::Array
        | FbsBaseType::Vector
        | FbsBaseType::Vector64 => unreachable!("{outer_type:#?} into {inner_type:#?}"),
        // NOTE: `FbsType` isn't actually an enum, it's just a bunch of constants…
        _ => unreachable!("{inner_type:#?}"),
    }
}

fn try_get_enum_fqname(
    enums: &[FbsEnum<'_>],
    field_type: FbsType<'_>,
    typ: FbsBaseType,
    virtpath: &str,
) -> Option<String> {
    if is_int(typ) {
        // Hack needed because enums get `typ == FbsBaseType::Byte`,
        // or whatever integer type the enum was assigned to.
        let enum_index = field_type.index() as usize;
        if enum_index < enums.len() {
            // It is an enum.
            assert!(
                is_uint(typ),
                "{virtpath}: For consistency, enums must be unsigned integers, i.e. `ubyte`, `ushort`, `uint` or `ulong`"
            );

            let enum_ = &enums[field_type.index() as usize];
            return Some(enum_.name().to_owned());
        }
    }
    None
}

fn is_int(typ: FbsBaseType) -> bool {
    matches!(
        typ,
        FbsBaseType::Byte
            | FbsBaseType::UByte
            | FbsBaseType::Short
            | FbsBaseType::UShort
            | FbsBaseType::Int
            | FbsBaseType::UInt
            | FbsBaseType::Long
            | FbsBaseType::ULong
    )
}

fn is_uint(typ: FbsBaseType) -> bool {
    matches!(
        typ,
        FbsBaseType::UByte | FbsBaseType::UShort | FbsBaseType::UInt | FbsBaseType::ULong
    )
}

fn attributes_from_raw_attrs(
    attrs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<FbsKeyValue<'_>>>>,
) -> Attributes {
    Attributes(
        attrs
            .into_iter()
            .flatten()
            .map(|kv| (kv.key().to_owned(), kv.value().map(ToOwned::to_owned)))
            .collect(),
    )
}

fn docs_from_raw_docs(
    reporter: &Reporter,
    virtpath: &str,
    fqname: &str,
    docs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&'_ str>>>,
) -> Docs {
    Docs::from_lines(
        reporter,
        virtpath,
        fqname,
        docs.into_iter().flat_map(|doc| doc.into_iter()),
    )
}

fn filepath_from_declaration_file(
    include_dir_path: impl AsRef<Utf8Path>,
    declaration_file: impl AsRef<str>,
) -> Utf8PathBuf {
    // It seems fbs is *very* confused about UNC paths on windows!
    let declaration_file = declaration_file.as_ref();
    let declaration_file = declaration_file
        .strip_prefix("//")
        .map_or(declaration_file, |f| {
            f.trim_start_matches("../").trim_start_matches("/?/")
        });

    let declaration_file = Utf8PathBuf::from(declaration_file);
    let declaration_file = if declaration_file.is_absolute() {
        declaration_file
    } else {
        include_dir_path
            .as_ref()
            .join(crate::format_path(&declaration_file))
    };

    declaration_file
        .canonicalize_utf8()
        .unwrap_or_else(|_| panic!("Failed to canonicalize declaration path {declaration_file:?}"))
}
