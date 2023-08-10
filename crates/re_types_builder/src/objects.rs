//! This package implements the semantic pass of the codegen process.
//!
//! The semantic pass transforms the low-level raw reflection data into higher level types that
//! are much easier to inspect and manipulate / friendler to work with.

use std::collections::{HashMap, HashSet};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;

use crate::{
    root_as_schema, FbsBaseType, FbsEnum, FbsEnumVal, FbsField, FbsKeyValue, FbsObject, FbsSchema,
    FbsType,
};

// ---

/// The result of the semantic pass: an intermediate representation of all available object
/// types; including structs, enums and unions.
#[derive(Debug)]
pub struct Objects {
    /// Maps fully-qualified type names to their resolved object definitions.
    pub objects: HashMap<String, Object>,
}

impl Objects {
    /// Runs the semantic pass on a serialized flatbuffers schema.
    ///
    /// The buffer must be a serialized [`FbsSchema`] (i.e. `.bfbs` data).
    pub fn from_buf(include_dir_path: impl AsRef<Utf8Path>, buf: &[u8]) -> Self {
        let schema = root_as_schema(buf).unwrap();
        Self::from_raw_schema(include_dir_path, &schema)
    }

    /// Runs the semantic pass on a deserialized flatbuffers [`FbsSchema`].
    pub fn from_raw_schema(include_dir_path: impl AsRef<Utf8Path>, schema: &FbsSchema<'_>) -> Self {
        let mut resolved_objs = HashMap::new();
        let mut resolved_enums = HashMap::new();

        let enums = schema.enums().iter().collect::<Vec<_>>();
        let objs = schema.objects().iter().collect::<Vec<_>>();

        let include_dir_path = include_dir_path.as_ref();

        // resolve enums
        for enm in schema.enums() {
            let resolved_enum = Object::from_raw_enum(include_dir_path, &enums, &objs, &enm);
            resolved_enums.insert(resolved_enum.fqname.clone(), resolved_enum);
        }

        // resolve objects
        for obj in schema.objects().iter() {
            let resolved_obj = Object::from_raw_object(include_dir_path, &enums, &objs, &obj);
            resolved_objs.insert(resolved_obj.fqname.clone(), resolved_obj);
        }

        let mut this = Self {
            objects: resolved_enums.into_iter().chain(resolved_objs).collect(),
        };

        // Validate fields types: Archetype consist of components, everything else consists of datatypes.
        for obj in this.objects.values() {
            for field in &obj.fields {
                if let Some(target_fqname) = field.typ.fqname() {
                    let target_obj = &this[target_fqname];
                    if obj.kind == ObjectKind::Archetype {
                        assert!(target_obj.kind == ObjectKind::Component,
                            "Field {:?} (pointing to an instance of {:?}) is part of an archetypes but is not a component. Only components are allowed as fields on an Archetype.",
                            field.fqname, target_fqname
                        );
                    } else {
                        assert!(target_obj.kind == ObjectKind::Datatype,
                            "Field {:?} (pointing to an instance of {:?}) is part of a Component or Datatype but is itself not a Datatype. Only Archetype fields can be Components, all other fields have to be primitive or be a datatypes.",
                            field.fqname, target_fqname
                        );
                    }
                } else {
                    // Note that we *do* allow primitive fields on components for the moment. Not doing so creates a lot of bloat.
                    assert!(obj.kind != ObjectKind::Archetype,
                        "Field {:?} is a primitive field which is part of an Archetype. Only Components are allowed on Archetypes.",
                        field.fqname);
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
                                virtpath: _,
                                filepath: _,
                                fqname,
                                pkg_name: _,
                                name: _,
                                docs: _,
                                typ,
                                attrs,
                                order: _,
                                is_nullable: _,
                                is_deprecated: _,
                                datatype,
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
        this.objects = this
            .objects
            .drain()
            .filter(|(_, obj)| !obj.is_transparent())
            .collect();

        this
    }
}

impl Objects {
    /// Returns all available objects, pre-sorted in ascending order based on their `order`
    /// attribute.
    pub fn ordered_objects_mut(&mut self, kind: Option<ObjectKind>) -> Vec<&mut Object> {
        let objs = self
            .objects
            .values_mut()
            .filter(|obj| kind.map_or(true, |kind| obj.kind == kind));

        let mut objs = objs.collect::<Vec<_>>();
        objs.sort_by_key(|anyobj| anyobj.order);

        objs
    }

    /// Returns all available objects, pre-sorted in ascending order based on their `order`
    /// attribute.
    pub fn ordered_objects(&self, kind: Option<ObjectKind>) -> Vec<&Object> {
        let objs = self
            .objects
            .values()
            .filter(|obj| kind.map_or(true, |kind| obj.kind == kind));

        let mut objs = objs.collect::<Vec<_>>();
        objs.sort_by_key(|anyobj| anyobj.order);

        objs
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
/// let obj = &objects["rerun.components.Label"];
/// let obj = &objects["rerun.archetypes.Point2D"];
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
    pub fn from_pkg_name(pkg_name: impl AsRef<str>) -> Self {
        let pkg_name = pkg_name.as_ref().replace(".testing", "");
        if pkg_name.starts_with("rerun.datatypes") {
            ObjectKind::Datatype
        } else if pkg_name.starts_with("rerun.components") {
            ObjectKind::Component
        } else if pkg_name.starts_with("rerun.archetypes") {
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
}

/// A high-level representation of a flatbuffers object's documentation.
#[derive(Debug, Clone)]
pub struct Docs {
    /// General documentation for the object.
    ///
    /// Each entry in the vector is a raw line, extracted as-is from the fbs definition.
    /// Trim it yourself if needed!
    ///
    /// See also [`Docs::tagged_docs`].
    pub doc: Vec<String>,

    /// Tagged documentation for the object.
    ///
    /// Each entry maps a tag value to a bunch of lines.
    /// Each entry in the vector is a raw line, extracted as-is from the fbs definition.
    /// Trim it yourself if needed!
    ///
    /// E.g. the following will be associated with the `py` tag:
    /// ```flatbuffers
    /// /// \py Something something about how this fields behave in python.
    /// my_field: uint32,
    /// ```
    ///
    /// See also [`Docs::doc`].
    pub tagged_docs: HashMap<String, Vec<String>>,

    /// Contents of all the files included using `\include:<path>`.
    pub included_files: HashMap<Utf8PathBuf, String>,
}

impl Docs {
    fn from_raw_docs(
        filepath: &Utf8Path,
        docs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&'_ str>>>,
    ) -> Self {
        let mut included_files = HashMap::default();

        let include_file = |included_files: &mut HashMap<_, _>, raw_path: &str| {
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
                if let Some((_, path)) = line.split_once("\\include:") {
                    include_file(&mut included_files, path)
                        .lines()
                        .map(|line| line.to_owned())
                        .collect_vec()
                } else {
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
                        (tag[1..].to_owned(), line.to_owned())
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
            let mut tagged_docs = HashMap::new();

            for cur_tag in all_tags {
                tagged_docs.insert(
                    cur_tag.clone(),
                    tagged_lines
                        .iter()
                        .filter_map(|(tag, line)| (cur_tag == tag).then(|| line.clone()))
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

    /// Fully-qualified name of the object, e.g. `rerun.components.Point2D`.
    pub fqname: String,

    /// Fully-qualified package name of the object, e.g. `rerun.components`.
    pub pkg_name: String,

    /// Name of the object, e.g. `Point2D`.
    pub name: String,

    /// The object's multiple layers of documentation.
    pub docs: Docs,

    /// The object's kind: datatype, component or archetype.
    pub kind: ObjectKind,

    /// The object's attributes.
    pub attrs: Attributes,

    /// The object's `order` attribute's value, which is always mandatory.
    pub order: u32,

    /// The object's inner fields, which can be either struct members or union values.
    ///
    /// These are pre-sorted, in ascending order, using their `order` attribute.
    pub fields: Vec<ObjectField>,

    /// Properties that only apply to either structs or unions.
    pub specifics: ObjectSpecifics,

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
        let (pkg_name, name) = fqname
            .rsplit_once('.')
            .map_or((String::new(), fqname.clone()), |(pkg_name, name)| {
                (pkg_name.to_owned(), name.to_owned())
            });

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
        let kind = ObjectKind::from_pkg_name(&pkg_name);
        let attrs = Attributes::from_raw_attrs(obj.attributes());
        let order = attrs.get::<u32>(&fqname, crate::ATTR_ORDER);

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
            fields.sort_by_key(|field| field.order);
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
            order,
            fields,
            specifics: ObjectSpecifics::Struct {},
            datatype: None,
        }
    }

    /// Resolves a raw [`FbsEnum`] into a higher-level representation that can be easily
    /// interpreted and manipulated.
    pub fn from_raw_enum(
        include_dir_path: impl AsRef<Utf8Path>,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        enm: &FbsEnum<'_>,
    ) -> Self {
        let include_dir_path = include_dir_path.as_ref();

        let fqname = enm.name().to_owned();
        let (pkg_name, name) = fqname
            .rsplit_once('.')
            .map_or((String::new(), fqname.clone()), |(pkg_name, name)| {
                (pkg_name.to_owned(), name.to_owned())
            });

        let virtpath = enm
            .declaration_file()
            .map(ToOwned::to_owned)
            .with_context(|| format!("no declaration_file found for {fqname}"))
            .unwrap();
        let filepath = filepath_from_declaration_file(include_dir_path, &virtpath);

        let docs = Docs::from_raw_docs(&filepath, enm.documentation());
        let kind = ObjectKind::from_pkg_name(&pkg_name);

        let utype = {
            if enm.underlying_type().base_type() == FbsBaseType::UType {
                // This is a union.
                None
            } else {
                Some(ElementType::from_raw_base_type(
                    enums,
                    objs,
                    enm.underlying_type(),
                    enm.underlying_type().base_type(),
                ))
            }
        };
        let attrs = Attributes::from_raw_attrs(enm.attributes());
        let order = attrs.get::<u32>(&fqname, crate::ATTR_ORDER);

        let fields: Vec<_> = enm
            .values()
            .iter()
            // NOTE: `BaseType::None` is only used by internal flatbuffers fields, we don't care.
            .filter(|val| {
                val.union_type()
                    .filter(|utype| utype.base_type() != FbsBaseType::None)
                    .is_some()
            })
            .map(|val| ObjectField::from_raw_enum_value(include_dir_path, enums, objs, enm, &val))
            .collect();

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
            order,
            fields,
            specifics: ObjectSpecifics::Union { utype },
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

    pub fn is_struct(&self) -> bool {
        match &self.specifics {
            ObjectSpecifics::Struct {} => true,
            ObjectSpecifics::Union { utype: _ } => false,
        }
    }

    pub fn is_enum(&self) -> bool {
        match &self.specifics {
            ObjectSpecifics::Struct {} => false,
            ObjectSpecifics::Union { utype } => utype.is_some(),
        }
    }

    pub fn is_union(&self) -> bool {
        match &self.specifics {
            ObjectSpecifics::Struct {} => false,
            ObjectSpecifics::Union { utype } => utype.is_none(),
        }
    }

    pub fn is_arrow_transparent(&self) -> bool {
        self.kind == ObjectKind::Component
            || self
                .try_get_attr::<String>(crate::ATTR_ARROW_TRANSPARENT)
                .is_some()
    }

    fn is_transparent(&self) -> bool {
        self.attrs
            .try_get::<String>(&self.fqname, crate::ATTR_TRANSPARENT)
            .is_some()
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
}

/// Properties specific to either structs or unions, but not both.
#[derive(Debug, Clone)]
pub enum ObjectSpecifics {
    Struct,
    Union {
        /// The underlying type of the union.
        ///
        /// `None` if this is a union, some value if this is an enum.
        utype: Option<ElementType>,
    },
}

/// A high-level representation of a flatbuffers field, which can be either a struct member or a
/// union value.
#[derive(Debug, Clone)]
pub struct ObjectField {
    /// Utf8Path of the associated fbs definition in the Flatbuffers hierarchy, e.g. `//rerun/components/point2d.fbs`.
    pub virtpath: String,

    /// Absolute filepath of the associated fbs definition.
    pub filepath: Utf8PathBuf,

    /// Fully-qualified name of the field, e.g. `rerun.components.Point2D#position`.
    pub fqname: String,

    /// Fully-qualified package name of the field, e.g. `rerun.components`.
    pub pkg_name: String,

    /// Name of the field, e.g. `x`.
    pub name: String,

    /// The field's multiple layers of documentation.
    pub docs: Docs,

    /// The field's type.
    pub typ: Type,

    /// The field's attributes.
    pub attrs: Attributes,

    /// The field's `order` attribute's value, which is always mandatory.
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

        let typ = Type::from_raw_type(enums, objs, field.type_());
        let attrs = Attributes::from_raw_attrs(field.attributes());
        let order = attrs.get::<u32>(&fqname, crate::ATTR_ORDER);

        let is_nullable = attrs
            .try_get::<String>(&fqname, crate::ATTR_NULLABLE)
            .is_some();
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

        let typ = Type::from_raw_type(
            enums,
            objs,
            // NOTE: Unwrapping is safe, we never resolve enums without union types.
            val.union_type().unwrap(),
        );

        let attrs = Attributes::from_raw_attrs(val.attributes());
        let order = attrs.get::<u32>(&fqname, crate::ATTR_ORDER);

        let is_nullable = attrs
            .try_get::<String>(&fqname, crate::ATTR_NULLABLE)
            .is_some();
        // TODO(cmc): not sure about this, but fbs unions are a bit weird that way
        let is_deprecated = false;

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

    fn is_transparent(&self) -> bool {
        self.attrs
            .try_get::<String>(&self.fqname, crate::ATTR_TRANSPARENT)
            .is_some()
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
}

/// The underlying type of an [`ObjectField`].
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
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
    ) -> Self {
        let typ = field_type.base_type();
        match typ {
            FbsBaseType::Bool => Self::Bool,
            FbsBaseType::Byte => Self::Int8,
            FbsBaseType::UByte => Self::UInt8,
            FbsBaseType::Short => Self::Int16,
            FbsBaseType::UShort => Self::UInt16,
            FbsBaseType::Int => Self::Int32,
            FbsBaseType::UInt => Self::UInt32,
            FbsBaseType::Long => Self::Int64,
            FbsBaseType::ULong => Self::UInt64,
            // TODO(cmc): half support
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
                ),
                length: field_type.fixed_length() as usize,
            },
            FbsBaseType::Vector => Self::Vector {
                elem_type: ElementType::from_raw_base_type(
                    enums,
                    objs,
                    field_type,
                    field_type.element(),
                ),
            },
            FbsBaseType::None | FbsBaseType::UType | FbsBaseType::Vector64 => {
                unimplemented!("{typ:#?}")
            }
            // NOTE: `FbsBaseType` isn't actually an enum, it's just a bunch of constants...
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
    ) -> Self {
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
            // NOTE: `FbsType` isn't actually an enum, it's just a bunch of constants...
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
}

// --- Common ---

/// A collection of arbitrary attributes.
#[derive(Debug, Default, Clone)]
pub struct Attributes(HashMap<String, Option<String>>);

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
                        .collect::<HashMap<_, _>>()
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
}

fn filepath_from_declaration_file(
    include_dir_path: impl AsRef<Utf8Path>,
    declaration_file: impl AsRef<str>,
) -> Utf8PathBuf {
    include_dir_path.as_ref().join("rerun").join(
        Utf8PathBuf::from(declaration_file.as_ref())
            .to_string()
            .replace("//", ""),
    )
}
