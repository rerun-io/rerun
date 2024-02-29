//! This package implements the semantic pass of the codegen process.
//!
//! The semantic pass transforms the low-level raw reflection data into higher level types that
//! are much easier to inspect and manipulate / friendler to work with.

use std::collections::{BTreeMap, HashSet};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;

use crate::{
    root_as_schema, FbsBaseType, FbsEnum, FbsEnumVal, FbsField, FbsKeyValue, FbsObject, FbsSchema,
    FbsType, Reporter, ATTR_IS_ENUM, ATTR_RERUN_OVERRIDE_TYPE,
};

// ---

/// The result of the semantic pass: an intermediate representation of all available object
/// types; including structs, enums and unions.
#[derive(Debug)]
pub struct Objects {
    /// Maps fully-qualified type names to their resolved object definitions.
    pub objects: BTreeMap<String, Object>,
}

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
        let mut resolved_objs = BTreeMap::new();
        let mut resolved_enums = BTreeMap::new();

        let enums = schema.enums().iter().collect::<Vec<_>>();
        let objs = schema.objects().iter().collect::<Vec<_>>();

        let include_dir_path = include_dir_path.as_ref();

        // resolve enums
        for enm in schema.enums() {
            let resolved_enum =
                Object::from_raw_enum(reporter, include_dir_path, &enums, &objs, &enm);
            resolved_enums.insert(resolved_enum.fqname.clone(), resolved_enum);
        }

        // resolve objects
        for obj in schema.objects() {
            let resolved_obj = Object::from_raw_object(include_dir_path, &enums, &objs, &obj);
            resolved_objs.insert(resolved_obj.fqname.clone(), resolved_obj);
        }

        let mut this = Self {
            objects: resolved_enums.into_iter().chain(resolved_objs).collect(),
        };

        // Validate fields types: Archetype consist of components, everything else consists of datatypes.
        for obj in this.objects.values() {
            for field in &obj.fields {
                let virtpath = &field.virtpath;
                if let Some(field_type_fqname) = field.typ.fqname() {
                    let field_obj = &this[field_type_fqname];
                    if obj.kind == ObjectKind::Archetype {
                        assert!(field_obj.kind == ObjectKind::Component,
                            "{virtpath}: Field {:?} (pointing to an instance of {:?}) is part of an archetypes but is not a component. Only components are allowed as fields on an Archetype.",
                            field.fqname, field_type_fqname
                        );
                    } else {
                        assert!(field_obj.kind == ObjectKind::Datatype,
                            "{virtpath}: Field {:?} (pointing to an instance of {:?}) is part of a Component or Datatype but is itself not a Datatype. Only Archetype fields can be Components, all other fields have to be primitive or be a datatypes.",
                            field.fqname, field_type_fqname
                        );
                    }
                } else {
                    // Note that we *do* allow primitive fields on components for the moment. Not doing so creates a lot of bloat.
                    assert!(obj.kind != ObjectKind::Archetype,
                        "{virtpath}: Field {:?} is a primitive field of type {:?}. Only Components are allowed on Archetypes. If this field is an enum, you need to set the {ATTR_IS_ENUM:?} attribute on the field.",
                        field.fqname, field.typ);
                }
            }
        }

        // Resolve field-level semantic transparency recursively.
        let mut done = false;
        while !done {
            done = true;
            let objects_copy = this.objects.clone(); // borrowck, the lazy way
            for obj in this.objects.values_mut() {
                for field in &mut obj.fields {
                    if field.is_transparent() {
                        if let Some(target_fqname) = field.typ.fqname() {
                            let mut target_obj = objects_copy[target_fqname].clone();
                            assert!(
                                target_obj.fields.len() == 1,
                                "field '{}' is marked transparent but points to object '{}' which \
                                    doesn't have exactly one field (found {} fields instead)",
                                field.fqname,
                                target_obj.fqname,
                                target_obj.fields.len(),
                            );

                            let ObjectField {
                                fqname,
                                typ,
                                attrs,
                                datatype,
                                ..
                            } = target_obj.fields.pop().unwrap();

                            field.typ = typ;
                            field.datatype = datatype;

                            // TODO(cmc): might want to do something smarter at some point regarding attrs.

                            // NOTE: Transparency (or lack thereof) of the target field takes precedence.
                            if let transparency @ Some(_) =
                                attrs.try_get::<String>(&fqname, crate::ATTR_TRANSPARENT)
                            {
                                field.attrs.0.insert(
                                    crate::ATTR_TRANSPARENT.to_owned(),
                                    transparency.clone(),
                                );
                            } else {
                                field.attrs.0.remove(crate::ATTR_TRANSPARENT);
                            }

                            done = false;
                        }
                    }
                }
            }
        }

        // Remove whole objects marked as transparent.
        this.objects.retain(|_, obj| !obj.is_transparent());

        this
    }
}

impl Objects {
    /// Returns all available objects, pre-sorted in ascending order based on their `order`
    /// attribute.
    pub fn ordered_objects_mut(&mut self, kind: Option<ObjectKind>) -> Vec<&mut Object> {
        self.objects
            .values_mut()
            .filter(|obj| kind.map_or(true, |kind| obj.kind == kind))
            .collect()
    }

    /// Returns all available objects, pre-sorted in ascending order based on their `order`
    /// attribute.
    pub fn ordered_objects(&self, kind: Option<ObjectKind>) -> Vec<&Object> {
        self.objects
            .values()
            .filter(|obj| kind.map_or(true, |kind| obj.kind == kind))
            .collect()
    }
}

/// Returns a resolved object using its fully-qualified name.
///
/// Panics if missing.
///
/// E.g.:
/// ```ignore
/// # let objects = Objects::default();
/// let obj = &objects["rerun.datatypes.Vec3D"];
/// let obj = &objects["rerun.datatypes.Angle"];
/// let obj = &objects["rerun.components.Text"];
/// let obj = &objects["rerun.archetypes.Position2D"];
/// ```
impl std::ops::Index<&str> for Objects {
    type Output = Object;

    fn index(&self, fqname: &str) -> &Self::Output {
        self.objects
            .get(fqname)
            .unwrap_or_else(|| panic!("unknown object: {fqname:?}"))
    }
}

// ---

/// The kind of the object, as determined by its package root (e.g. `rerun.components`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Datatype,
    Component,
    Archetype,
}

impl ObjectKind {
    pub const ALL: [Self; 3] = [Self::Datatype, Self::Component, Self::Archetype];

    // TODO(#2364): use an attr instead of the path
    pub fn from_pkg_name(pkg_name: &str, attrs: &Attributes) -> Self {
        assert!(!pkg_name.is_empty(), "Missing package name");

        let scope = match attrs.try_get::<String>(pkg_name, crate::ATTR_RERUN_SCOPE) {
            Some(scope) => format!(".{scope}"),
            None => String::new(),
        };

        let pkg_name = pkg_name.replace(".testing", "");
        if pkg_name.starts_with(format!("rerun{scope}.datatypes").as_str()) {
            ObjectKind::Datatype
        } else if pkg_name.starts_with(format!("rerun{scope}.components").as_str()) {
            ObjectKind::Component
        } else if pkg_name.starts_with(format!("rerun{scope}.archetypes").as_str()) {
            ObjectKind::Archetype
        } else {
            panic!("unknown package {pkg_name:?}");
        }
    }

    pub fn plural_snake_case(&self) -> &'static str {
        match self {
            ObjectKind::Datatype => "datatypes",
            ObjectKind::Component => "components",
            ObjectKind::Archetype => "archetypes",
        }
    }

    pub fn singular_name(&self) -> &'static str {
        match self {
            ObjectKind::Datatype => "Datatype",
            ObjectKind::Component => "Component",
            ObjectKind::Archetype => "Archetype",
        }
    }

    pub fn plural_name(&self) -> &'static str {
        match self {
            ObjectKind::Datatype => "Datatypes",
            ObjectKind::Component => "Components",
            ObjectKind::Archetype => "Archetypes",
        }
    }
}

/// A high-level representation of a flatbuffers object's documentation.
#[derive(Debug, Clone)]
pub struct Docs {
    /// General documentation for the object.
    ///
    /// Each entry in the vector is a line of comment,
    /// excluding the leading space end trailing newline,
    /// i.e. the `COMMENT` from `/// COMMENT\n`
    ///
    /// See also [`Docs::tagged_docs`].
    pub doc: Vec<String>,

    /// Tagged documentation for the object.
    ///
    /// Each entry in the vector is a line of comment,
    /// excluding the leading space end trailing newline,
    /// i.e. the `COMMENT` from `/// \py COMMENT\n`
    ///
    /// E.g. the following will be associated with the `py` tag:
    /// ```flatbuffers
    /// /// \py Something something about how this fields behave in python.
    /// my_field: uint32,
    /// ```
    ///
    /// See also [`Docs::doc`].
    pub tagged_docs: BTreeMap<String, Vec<String>>,

    /// Contents of all the files included using `\include:<path>`.
    pub included_files: BTreeMap<Utf8PathBuf, String>,
}

impl Docs {
    fn from_raw_docs(
        filepath: &Utf8Path,
        docs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&'_ str>>>,
    ) -> Self {
        let mut included_files = BTreeMap::default();

        let include_file = |included_files: &mut BTreeMap<_, _>, raw_path: &str| {
            let path: Utf8PathBuf = raw_path
                .parse()
                .with_context(|| format!("couldn't parse included path: {raw_path:?}"))
                .unwrap();

            let path = filepath.parent().unwrap().join(path);

            included_files
                .entry(path.clone())
                .or_insert_with(|| {
                    std::fs::read_to_string(&path)
                        .with_context(|| {
                            format!("couldn't parse read file at included path: {path:?}")
                        })
                        .unwrap()
                })
                .clone()
        };

        // language-agnostic docs
        let doc = docs
            .into_iter()
            .flat_map(|doc| doc.into_iter())
            // NOTE: discard tagged lines!
            .filter(|line| !line.trim().starts_with('\\'))
            .flat_map(|line| {
                assert!(!line.ends_with('\n'));
                assert!(!line.ends_with('\r'));

                if let Some((_, path)) = line.split_once("\\include:") {
                    include_file(&mut included_files, path)
                        .lines()
                        .map(|line| line.to_owned())
                        .collect_vec()
                } else if let Some(line) = line.strip_prefix(' ') {
                    // Removed space between `///` and comment.
                    vec![line.to_owned()]
                } else {
                    assert!(
                        line.is_empty(),
                        "{filepath}: Comments should start with a single space; found {line:?}"
                    );
                    vec![line.to_owned()]
                }
            })
            .collect::<Vec<_>>();

        // tagged docs, e.g. `\py this only applies to python!`
        let tagged_docs = {
            let tagged_lines = docs
                .into_iter()
                .flat_map(|doc| doc.into_iter())
                // NOTE: discard _un_tagged lines!
                .filter_map(|line| {
                    let trimmed = line.trim();
                    trimmed.starts_with('\\').then(|| {
                        let tag = trimmed.split_whitespace().next().unwrap();
                        let line = &trimmed[tag.len()..];
                        let tag = tag[1..].to_owned();
                        if let Some(line) = line.strip_prefix(' ') {
                            // Removed space between tag and comment.
                            (tag, line.to_owned())
                        } else {
                            assert!(line.is_empty());
                            (tag, String::default())
                        }
                    })
                })
                .flat_map(|(tag, line)| {
                    if let Some((_, path)) = line.split_once("\\include:") {
                        include_file(&mut included_files, path)
                            .lines()
                            .map(|line| (tag.clone(), line.to_owned()))
                            .collect_vec()
                    } else {
                        vec![(tag, line)]
                    }
                })
                .collect::<Vec<_>>();

            let all_tags: HashSet<_> = tagged_lines.iter().map(|(tag, _)| tag).collect();
            let mut tagged_docs = BTreeMap::new();

            for cur_tag in all_tags {
                tagged_docs.insert(
                    cur_tag.clone(),
                    tagged_lines
                        .iter()
                        .filter(|(tag, _)| cur_tag == tag)
                        .map(|(_, line)| line.clone())
                        .collect(),
                );
            }

            tagged_docs
        };

        Self {
            doc,
            tagged_docs,
            included_files,
        }
    }
}

/// A high-level representation of a flatbuffers object, which can be either a struct, a union or
/// an enum.
#[derive(Debug, Clone)]
pub struct Object {
    /// Utf8Path of the associated fbs definition in the Flatbuffers hierarchy, e.g. `//rerun/components/point2d.fbs`.
    pub virtpath: String,

    /// Absolute filepath of the associated fbs definition.
    pub filepath: Utf8PathBuf,

    /// Fully-qualified name of the object, e.g. `rerun.components.Position2D`.
    pub fqname: String,

    /// Fully-qualified package name of the object, e.g. `rerun.components`.
    pub pkg_name: String,

    /// PascalCase name of the object type, e.g. `Position2D`.
    pub name: String,

    /// The object's multiple layers of documentation.
    pub docs: Docs,

    /// The object's kind: datatype, component or archetype.
    pub kind: ObjectKind,

    /// The object's attributes.
    pub attrs: Attributes,

    /// The object's inner fields, which can be either struct members or union/emum variants.
    ///
    /// These are ordered using their `order` attribute (structs),
    /// or in the same order that they appeared in the .fbs (enum/union).
    pub fields: Vec<ObjectField>,

    /// struct, enum, or union?
    pub class: ObjectClass,

    /// The Arrow datatype of this `Object`, or `None` if the object is Arrow-transparent.
    ///
    /// This is lazily computed when the parent object gets registered into the Arrow registry and
    /// will be `None` until then.
    pub datatype: Option<crate::LazyDatatype>,
}

impl Object {
    /// Resolves a raw [`crate::Object`] into a higher-level representation that can be easily
    /// interpreted and manipulated.
    pub fn from_raw_object(
        include_dir_path: impl AsRef<Utf8Path>,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        obj: &FbsObject<'_>,
    ) -> Self {
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

        let docs = Docs::from_raw_docs(&filepath, obj.documentation());
        let attrs = Attributes::from_raw_attrs(obj.attributes());
        let kind = ObjectKind::from_pkg_name(&pkg_name, &attrs);

        let fields: Vec<_> = {
            let mut fields: Vec<_> = obj
                .fields()
                .iter()
                // NOTE: These are intermediate fields used by flatbuffers internals, we don't care.
                .filter(|field| field.type_().base_type() != FbsBaseType::UType)
                .filter(|field| field.type_().element() != FbsBaseType::UType)
                .map(|field| {
                    ObjectField::from_raw_object_field(include_dir_path, enums, objs, obj, &field)
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

        Self {
            virtpath,
            filepath,
            fqname,
            pkg_name,
            name,
            docs,
            kind,
            attrs,
            fields,
            class: ObjectClass::Struct,
            datatype: None,
        }
    }

    /// Resolves a raw [`FbsEnum`] into a higher-level representation that can be easily
    /// interpreted and manipulated.
    pub fn from_raw_enum(
        reporter: &Reporter,
        include_dir_path: impl AsRef<Utf8Path>,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        enm: &FbsEnum<'_>,
    ) -> Self {
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

        let docs = Docs::from_raw_docs(&filepath, enm.documentation());
        let attrs = Attributes::from_raw_attrs(enm.attributes());
        let kind = ObjectKind::from_pkg_name(&pkg_name, &attrs);

        let is_enum = enm.underlying_type().base_type() != FbsBaseType::UType;

        let fields: Vec<_> = enm
            .values()
            .iter()
            .filter(|val| {
                // NOTE: `BaseType::None` is only used by internal flatbuffers fields, we don't care.
                is_enum
                    || val
                        .union_type()
                        .filter(|utype| utype.base_type() != FbsBaseType::None)
                        .is_some()
            })
            .map(|val| {
                ObjectField::from_raw_enum_value(reporter, include_dir_path, enums, objs, enm, &val)
            })
            .collect();

        Self {
            virtpath,
            filepath,
            fqname,
            pkg_name,
            name,
            docs,
            kind,
            attrs,
            fields,
            class: if is_enum {
                ObjectClass::Enum
            } else {
                ObjectClass::Union
            },
            datatype: None,
        }
    }

    pub fn get_attr<T>(&self, name: impl AsRef<str>) -> T
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.attrs.get(self.fqname.as_str(), name)
    }

    pub fn try_get_attr<T>(&self, name: impl AsRef<str>) -> Option<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.attrs.try_get(self.fqname.as_str(), name)
    }

    pub fn is_attr_set(&self, name: impl AsRef<str>) -> bool {
        self.attrs.has(name)
    }

    pub fn is_struct(&self) -> bool {
        self.class == ObjectClass::Struct
    }

    pub fn is_enum(&self) -> bool {
        self.class == ObjectClass::Enum
    }

    pub fn is_union(&self) -> bool {
        self.class == ObjectClass::Union
    }

    pub fn is_arrow_transparent(&self) -> bool {
        if self.is_enum() {
            return false; // Enums are encoded as sparse unions
        }
        self.kind == ObjectKind::Component || self.attrs.has(crate::ATTR_ARROW_TRANSPARENT)
    }

    fn is_transparent(&self) -> bool {
        self.attrs.has(crate::ATTR_TRANSPARENT)
    }

    /// Is the destructor trivial/default (i.e. is this simple data with no allocations)?
    pub fn has_default_destructor(&self, objects: &Objects) -> bool {
        self.fields
            .iter()
            .all(|field| field.typ.has_default_destructor(objects))
    }

    /// Try to find the relative file path of the `.fbs` source file.
    pub fn relative_filepath(&self) -> Option<&Utf8Path> {
        self.filepath
            .strip_prefix(crate::rerun_workspace_path())
            .ok()
    }

    /// The `snake_case` name of the object, e.g. `translation_and_mat3x3`.
    pub fn snake_case_name(&self) -> String {
        crate::to_snake_case(&self.name)
    }

    /// Returns true if this object is part of testing and not to be used in the production SDK.
    pub fn is_testing(&self) -> bool {
        is_testing_fqname(&self.fqname)
    }

    pub fn scope(&self) -> Option<String> {
        self.try_get_attr::<String>(crate::ATTR_RERUN_SCOPE)
    }

    pub fn deprecation_notice(&self) -> Option<String> {
        self.try_get_attr::<String>(crate::ATTR_RERUN_DEPRECATED)
    }

    /// Returns the crate name of an object, accounting for overrides.
    pub fn crate_name(&self) -> String {
        self.try_get_attr::<String>(crate::ATTR_RUST_OVERRIDE_CRATE)
            .unwrap_or_else(|| "re_types".to_owned())
    }

    /// Returns the module name of an object.
    //
    // NOTE: Might want a module override at some point.
    pub fn module_name(&self) -> String {
        if let Some(scope) = self.scope() {
            format!("{}/{}", scope, self.kind.plural_snake_case())
        } else {
            self.kind.plural_snake_case().to_owned()
        }
    }
}

pub fn is_testing_fqname(fqname: &str) -> bool {
    fqname.contains("rerun.testing")
}

/// Is this a struct, enum, or union?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectClass {
    Struct,

    /// Dumb C-style enum.
    ///
    /// Encoded as a sparse arrow union.
    ///
    /// Arrow uses a `i8` to encode the variant, forbidding negatives,
    /// so there are 127 possible states.
    /// We reserve `0` for a special/implicit `__null_markers` variant,
    /// which we use to encode null values.
    /// This means we support at most 126 possible enum variants.
    /// Therefore the enum can be backed by a simple `u8` in Rust and C++.
    Enum,

    /// Proper sum-type union.
    ///
    /// Encoded as a dense arrow union.
    ///
    /// Arrow uses a `i8` to encode the variant, forbidding negatives,
    /// so there are 127 possible states.
    /// We reserve `0` for a special/implicit `__null_markers` variant,
    /// which we use to encode null values.
    /// This means we support at most 126 possible union variants.
    Union,
}

/// A high-level representation of a flatbuffers field, which can be either a struct member or a
/// union value.
#[derive(Debug, Clone)]
pub struct ObjectField {
    /// Utf8Path of the associated fbs definition in the Flatbuffers hierarchy, e.g. `//rerun/components/point2d.fbs`.
    pub virtpath: String,

    /// Absolute filepath of the associated fbs definition.
    pub filepath: Utf8PathBuf,

    /// Fully-qualified name of the field, e.g. `rerun.components.Position2D#position`.
    pub fqname: String,

    /// Fully-qualified package name of the field, e.g. `rerun.components`.
    pub pkg_name: String,

    /// Name of the field, e.g. `x`.
    ///
    /// For struct fields this is usually `snake_case`,
    /// but for enums it is usually `PascalCase`.
    pub name: String,

    /// The field's multiple layers of documentation.
    pub docs: Docs,

    /// The field's type.
    pub typ: Type,

    /// The field's attributes.
    pub attrs: Attributes,

    /// The struct field's `order` attribute's value, which is mandatory for struct fields
    /// (otherwise their order is undefined).
    pub order: u32,

    /// Whether the field is nullable.
    pub is_nullable: bool,

    /// Whether the field is deprecated.
    //
    // TODO(#2366): do something with this
    // TODO(#2367): implement custom attr to specify deprecation reason
    pub is_deprecated: bool,

    /// The Arrow datatype of this `ObjectField`.
    ///
    /// This is lazily computed when the parent object gets registered into the Arrow registry and
    /// will be `None` until then.
    pub datatype: Option<crate::LazyDatatype>,
}

impl ObjectField {
    pub fn from_raw_object_field(
        include_dir_path: impl AsRef<Utf8Path>,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        obj: &FbsObject<'_>,
        field: &FbsField<'_>,
    ) -> Self {
        let fqname = format!("{}#{}", obj.name(), field.name());
        let (pkg_name, name) = fqname
            .rsplit_once('#')
            .map_or((String::new(), fqname.clone()), |(pkg_name, name)| {
                (pkg_name.to_owned(), name.to_owned())
            });

        let virtpath = obj
            .declaration_file()
            .map(ToOwned::to_owned)
            .with_context(|| format!("no declaration_file found for {fqname}"))
            .unwrap();
        let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);

        let docs = Docs::from_raw_docs(&filepath, field.documentation());

        let attrs = Attributes::from_raw_attrs(field.attributes());

        let typ = Type::from_raw_type(enums, objs, field.type_(), &attrs);
        let order = attrs.get::<u32>(&fqname, crate::ATTR_ORDER);

        let is_nullable = attrs.has(crate::ATTR_NULLABLE);
        let is_deprecated = field.deprecated();

        Self {
            virtpath,
            filepath,
            fqname,
            pkg_name,
            name,
            docs,
            typ,
            attrs,
            order,
            is_nullable,
            is_deprecated,
            datatype: None,
        }
    }

    pub fn from_raw_enum_value(
        reporter: &Reporter,
        include_dir_path: impl AsRef<Utf8Path>,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        enm: &FbsEnum<'_>,
        val: &FbsEnumVal<'_>,
    ) -> Self {
        let fqname = format!("{}#{}", enm.name(), val.name());
        let (pkg_name, name) = fqname
            .rsplit_once('#')
            .map_or((String::new(), fqname.clone()), |(pkg_name, name)| {
                (pkg_name.to_owned(), name.to_owned())
            });

        let virtpath = enm
            .declaration_file()
            .map(ToOwned::to_owned)
            .with_context(|| format!("no declaration_file found for {fqname}"))
            .unwrap();
        let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);

        let docs = Docs::from_raw_docs(&filepath, val.documentation());

        let attrs = Attributes::from_raw_attrs(val.attributes());

        let typ = Type::from_raw_type(
            enums,
            objs,
            // NOTE: Unwrapping is safe, we never resolve enums without union types.
            val.union_type().unwrap(),
            &attrs,
        );

        let is_nullable = attrs.has(crate::ATTR_NULLABLE);
        // TODO(cmc): not sure about this, but fbs unions are a bit weird that way
        let is_deprecated = false;

        if attrs.has(crate::ATTR_ORDER) {
            reporter.warn(
                &virtpath,
                &fqname,
                "There is no need for an `order` attribute on enum/union variants",
            );
        }

        Self {
            virtpath,
            filepath,
            fqname,
            pkg_name,
            name,
            docs,
            typ,
            attrs,
            order: 0, // no needed for enums
            is_nullable,
            is_deprecated,
            datatype: None,
        }
    }

    fn is_transparent(&self) -> bool {
        self.attrs.has(crate::ATTR_TRANSPARENT)
    }

    pub fn get_attr<T>(&self, name: impl AsRef<str>) -> T
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.attrs.get(self.fqname.as_str(), name)
    }

    pub fn try_get_attr<T>(&self, name: impl AsRef<str>) -> Option<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.attrs.try_get(self.fqname.as_str(), name)
    }

    pub fn has_attr(&self, name: impl AsRef<str>) -> bool {
        self.attrs.has(name)
    }

    /// The `snake_case` name of the field, e.g. `translation_and_mat3x3`.
    pub fn snake_case_name(&self) -> String {
        crate::to_snake_case(&self.name)
    }

    /// The `PascalCase` name of the field, e.g. `TranslationAndMat3x3`.
    pub fn pascal_case_name(&self) -> String {
        crate::to_pascal_case(&self.name)
    }

    /// Returns true if this object is part of testing and not to be used in the production SDK.
    pub fn is_testing(&self) -> bool {
        is_testing_fqname(&self.fqname)
    }

    pub fn kind(&self) -> Option<FieldKind> {
        if self.has_attr(crate::ATTR_RERUN_COMPONENT_REQUIRED) {
            Some(FieldKind::Required)
        } else if self.has_attr(crate::ATTR_RERUN_COMPONENT_RECOMMENDED) {
            Some(FieldKind::Recommended)
        } else if self.has_attr(crate::ATTR_RERUN_COMPONENT_OPTIONAL) {
            Some(FieldKind::Optional)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Required,
    Recommended,
    Optional,
}

/// The underlying type of an [`ObjectField`].
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    /// This is the unit type, used for `enum` variants.
    ///
    /// In `arrow`, this corresponds to the `null` type`.
    ///
    /// In rust this would be `()`, and in C++ this would be `void`.
    Unit,

    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Int8,
    Int16,
    Int32,
    Int64,
    Bool,
    Float16,
    Float32,
    Float64,
    String,
    Array {
        elem_type: ElementType,
        length: usize,
    },
    Vector {
        elem_type: ElementType,
    },
    Object(String), // fqname
}

impl From<ElementType> for Type {
    fn from(typ: ElementType) -> Self {
        match typ {
            ElementType::UInt8 => Self::UInt8,
            ElementType::UInt16 => Self::UInt16,
            ElementType::UInt32 => Self::UInt32,
            ElementType::UInt64 => Self::UInt64,
            ElementType::Int8 => Self::Int8,
            ElementType::Int16 => Self::Int16,
            ElementType::Int32 => Self::Int32,
            ElementType::Int64 => Self::Int64,
            ElementType::Bool => Self::Bool,
            ElementType::Float16 => Self::Float16,
            ElementType::Float32 => Self::Float32,
            ElementType::Float64 => Self::Float64,
            ElementType::String => Self::String,
            ElementType::Object(fqname) => Self::Object(fqname),
        }
    }
}

impl Type {
    pub fn from_raw_type(
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        field_type: FbsType<'_>,
        attrs: &Attributes,
    ) -> Self {
        // TODO(jleibs): Clean up fqname plumbing
        let fqname = "???";

        let typ = field_type.base_type();

        if let Some(type_override) = attrs.try_get::<String>(fqname, ATTR_RERUN_OVERRIDE_TYPE) {
            match (typ, type_override.as_str()) {
                (FbsBaseType::UShort, "float16") => {
                    return Self::Float16;
                },
                (FbsBaseType::Array | FbsBaseType::Vector, "float16") => {}
                _ => unreachable!("UShort -> float16 is the only permitted type override. Not {typ:#?}->{type_override}"),
            }
        }

        if attrs.has(crate::ATTR_IS_ENUM) {
            // Hack needed because enums get `typ == FbsBaseType::Byte`.
            let enum_type = enums[field_type.index() as usize].name();
            return Type::Object(enum_type.to_owned());
        }

        match typ {
            // Enum variant
            FbsBaseType::None => Self::Unit,

            FbsBaseType::Bool => Self::Bool,
            FbsBaseType::Byte => Self::Int8,
            FbsBaseType::UByte => Self::UInt8,
            FbsBaseType::Short => Self::Int16,
            FbsBaseType::UShort => Self::UInt16,
            FbsBaseType::Int => Self::Int32,
            FbsBaseType::UInt => Self::UInt32,
            FbsBaseType::Long => Self::Int64,
            FbsBaseType::ULong => Self::UInt64,
            FbsBaseType::Float => Self::Float32,
            FbsBaseType::Double => Self::Float64,
            FbsBaseType::String => Self::String,
            FbsBaseType::Obj => {
                let obj = &objs[field_type.index() as usize];
                Self::Object(obj.name().to_owned())
            }
            FbsBaseType::Union => {
                let union = &enums[field_type.index() as usize];
                Self::Object(union.name().to_owned())
            }
            FbsBaseType::Array => Self::Array {
                elem_type: ElementType::from_raw_base_type(
                    enums,
                    objs,
                    field_type,
                    field_type.element(),
                    attrs,
                ),
                length: field_type.fixed_length() as usize,
            },
            FbsBaseType::Vector => Self::Vector {
                elem_type: ElementType::from_raw_base_type(
                    enums,
                    objs,
                    field_type,
                    field_type.element(),
                    attrs,
                ),
            },
            FbsBaseType::UType | FbsBaseType::Vector64 => {
                unimplemented!("FbsBaseType::{typ:#?}")
            }
            // NOTE: `FbsBaseType` isn't actually an enum, it's just a bunch of constants…
            _ => unreachable!("{typ:#?}"),
        }
    }

    /// True if this is some kind of array/vector.
    pub fn is_plural(&self) -> bool {
        self.plural_inner().is_some()
    }

    /// Returns element type for arrays and vectors.
    pub fn plural_inner(&self) -> Option<&ElementType> {
        match self {
            Self::Vector { elem_type }
            | Self::Array {
                elem_type,
                length: _,
            } => Some(elem_type),

            Self::Unit
            | Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::Bool
            | Self::Float16
            | Self::Float32
            | Self::Float64
            | Self::String
            | Self::Object(_) => None,
        }
    }

    pub fn vector_inner(&self) -> Option<&ElementType> {
        self.plural_inner()
            .filter(|_| matches!(self, Self::Vector { .. }))
    }

    /// `Some(fqname)` if this is an `Object` or an `Array`/`Vector` of `Object`s.
    pub fn fqname(&self) -> Option<&str> {
        match self {
            Self::Object(fqname) => Some(fqname.as_str()),
            Self::Array {
                elem_type,
                length: _,
            }
            | Self::Vector { elem_type } => elem_type.fqname(),
            _ => None,
        }
    }

    /// Is the destructor trivial/default (i.e. is this simple data with no allocations)?
    pub fn has_default_destructor(&self, objects: &Objects) -> bool {
        match self {
            Self::Unit
            | Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::Bool
            | Self::Float16
            | Self::Float32
            | Self::Float64 => true,

            Self::String | Self::Vector { .. } => false,

            Self::Array { elem_type, .. } => elem_type.has_default_destructor(objects),

            Self::Object(fqname) => objects[fqname].has_default_destructor(objects),
        }
    }
}

/// The underlying element type for arrays/vectors/maps.
///
/// Flatbuffers doesn't support directly nesting multiple layers of arrays, they
/// always have to be wrapped into intermediate layers of structs or tables!
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ElementType {
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Int8,
    Int16,
    Int32,
    Int64,
    Bool,
    Float16,
    Float32,
    Float64,
    String,
    Object(String), // fqname
}

impl ElementType {
    pub fn from_raw_base_type(
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        outer_type: FbsType<'_>,
        inner_type: FbsBaseType,
        attrs: &Attributes,
    ) -> Self {
        // TODO(jleibs): Clean up fqname plumbing
        let fqname = "???";
        if let Some(type_override) = attrs.try_get::<String>(fqname, ATTR_RERUN_OVERRIDE_TYPE) {
            match (inner_type, type_override.as_str()) {
                (FbsBaseType::UShort, "float16") => {
                    return Self::Float16;
                }
                _ => unreachable!("UShort -> float16 is the only permitted type override. Not {inner_type:#?}->{type_override}"),
            }
        }

        #[allow(clippy::match_same_arms)]
        match inner_type {
            FbsBaseType::Bool => Self::Bool,
            FbsBaseType::Byte => Self::Int8,
            FbsBaseType::UByte => Self::UInt8,
            FbsBaseType::Short => Self::Int16,
            FbsBaseType::UShort => Self::UInt16,
            FbsBaseType::Int => Self::Int32,
            FbsBaseType::UInt => Self::UInt32,
            FbsBaseType::Long => Self::Int64,
            FbsBaseType::ULong => Self::UInt64,
            FbsBaseType::Float => Self::Float32,
            FbsBaseType::Double => Self::Float64,
            FbsBaseType::String => Self::String,
            FbsBaseType::Obj => {
                let obj = &objs[outer_type.index() as usize];
                Self::Object(obj.name().to_owned())
            }
            FbsBaseType::Union => {
                let enm = &enums[outer_type.index() as usize];
                Self::Object(enm.name().to_owned())
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

    /// `Some(fqname)` if this is an `Object`.
    pub fn fqname(&self) -> Option<&str> {
        match self {
            Self::Object(fqname) => Some(fqname.as_str()),
            _ => None,
        }
    }

    /// Is the destructor trivial/default (i.e. is this simple data with no allocations)?
    pub fn has_default_destructor(&self, objects: &Objects) -> bool {
        match self {
            Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::Bool
            | Self::Float16
            | Self::Float32
            | Self::Float64 => true,

            Self::String => false,

            Self::Object(fqname) => objects[fqname].has_default_destructor(objects),
        }
    }

    /// Is this type directly backed by a native arrow `Buffer`. This means the data can
    /// be returned using a `re_types::ArrowBuffer` which facilitates direct zero-copy access to
    /// a slice representation.
    pub fn backed_by_arrow_buffer(&self) -> bool {
        match self {
            Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64
            | Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::Float16
            | Self::Float32
            | Self::Float64 => true,
            Self::Bool | Self::Object(_) | Self::String => false,
        }
    }
}

// --- Common ---

/// A collection of arbitrary attributes.
#[derive(Debug, Default, Clone)]
pub struct Attributes(BTreeMap<String, Option<String>>);

impl Attributes {
    fn from_raw_attrs(
        attrs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<FbsKeyValue<'_>>>>,
    ) -> Self {
        Self(
            attrs
                .map(|attrs| {
                    attrs
                        .into_iter()
                        .map(|kv| (kv.key().to_owned(), kv.value().map(ToOwned::to_owned)))
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default(),
        )
    }
}

impl Attributes {
    pub fn get<T>(&self, owner_fqname: impl AsRef<str>, name: impl AsRef<str>) -> T
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let owner_fqname = owner_fqname.as_ref();
        let name = name.as_ref();

        let value_str = self
            .0
            .get(name)
            .cloned() // cannot flatten it otherwise
            .flatten()
            .with_context(|| format!("no `{name}` attribute was specified for `{owner_fqname}`"))
            .unwrap();

        value_str
            .parse()
            .with_context(|| {
                let type_of_t = std::any::type_name::<T>();
                format!(
                    "invalid `{name}` attribute for `{owner_fqname}`: \
                    expected {type_of_t}, got `{value_str}` instead"
                )
            })
            .unwrap()
    }

    pub fn try_get<T>(&self, owner_fqname: impl AsRef<str>, name: impl AsRef<str>) -> Option<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let owner_fqname = owner_fqname.as_ref();
        let name = name.as_ref();

        let value_str = self
            .0
            .get(name)
            .cloned() // cannot flatten it otherwise
            .flatten()?;

        Some(
            value_str
                .parse()
                .with_context(|| {
                    let type_of_t = std::any::type_name::<T>();
                    format!(
                        "invalid `{name}` attribute for `{owner_fqname}`: \
                        expected {type_of_t}, got `{value_str}` instead"
                    )
                })
                .unwrap(),
        )
    }

    pub fn has(&self, name: impl AsRef<str>) -> bool {
        self.0.contains_key(name.as_ref())
    }
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
    if declaration_file.is_absolute() {
        declaration_file
    } else {
        include_dir_path
            .as_ref()
            .join("rerun")
            .join(crate::format_path(&declaration_file))
    }
    .canonicalize_utf8()
    .expect("Failed to canonicalize declaration path")
}
