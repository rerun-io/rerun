//! This package implements the semantic pass of the codegen process.
//!
//! The semantic pass transforms the low-level raw reflection data into higher level types that
//! are much easier to inspect and manipulate / friendler to work with.

use std::collections::BTreeMap;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools as _;

use crate::data_type::LazyDatatype;
use crate::{
    ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED, ATTR_RERUN_COMPONENT_REQUIRED,
    ATTR_RERUN_DEPRECATED_NOTICE, ATTR_RERUN_DEPRECATED_SINCE, ATTR_RERUN_OVERRIDE_TYPE,
    ATTR_RERUN_STATE, Docs, FbsBaseType, FbsEnum, FbsEnumVal, FbsField, FbsKeyValue, FbsObject,
    FbsSchema, FbsType, Reporter, root_as_schema,
};

// ---

const BUILTIN_UNIT_TYPE_FQNAME: &str = "rerun.builtins.UnitType";

/// The result of the semantic pass: an intermediate representation of all available object
/// types; including structs, enums and unions.
#[derive(Debug, Default)]
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
            if obj.name() == BUILTIN_UNIT_TYPE_FQNAME {
                continue;
            }

            let resolved_obj =
                Object::from_raw_object(reporter, include_dir_path, &enums, &objs, &obj);
            resolved_objs.insert(resolved_obj.fqname.clone(), resolved_obj);
        }

        let mut this = Self {
            objects: resolved_enums.into_iter().chain(resolved_objs).collect(),
        };

        // Validate fields types: Archetype consist of components, Views (aka SuperArchetypes) consist of archetypes, everything else consists of datatypes.
        for obj in this.objects.values() {
            for field in &obj.fields {
                let virtpath = &field.virtpath;
                if let Some(field_type_fqname) = field.typ.fqname() {
                    let field_obj = &this[field_type_fqname];
                    match obj.kind {
                        ObjectKind::Datatype | ObjectKind::Component => {
                            if field_obj.kind != ObjectKind::Datatype {
                                reporter.error(virtpath, field_type_fqname, "Is part of a Component or Datatype but is itself not a Datatype. Only archetype fields can be components, all other fields have to be primitive or be a datatypes.");
                            }
                        }
                        ObjectKind::Archetype => {
                            if field_obj.kind != ObjectKind::Component {
                                reporter.error(virtpath, field_type_fqname, "Is part of an archetype but is not a component. Only components are allowed as fields on an archetype.");
                            }

                            validate_archetype_field_attributes(reporter, obj);
                        }
                        ObjectKind::View => {
                            if field_obj.kind != ObjectKind::Archetype {
                                reporter.error(virtpath, field_type_fqname, "Is part of an view but is not an archetype. Only archetypes are allowed as fields of a view's properties.");
                            }
                        }
                    }
                } else if obj.kind != ObjectKind::Datatype {
                    let is_enum_component = obj.kind == ObjectKind::Component && obj.is_enum(); // Enum components are allowed to have no datatype.
                    let is_test_component = obj.kind == ObjectKind::Component && obj.is_testing(); // Test components are allowed to have datatypes for the moment. TODO(andreas): Should clean this up as well!
                    if !is_enum_component && !is_test_component {
                        reporter.error(virtpath, &obj.fqname, format!("Field {:?} s a primitive field of type {:?}. Primitive types are only allowed on DataTypes.", field.fqname, field.typ));
                    }
                }

                if obj.is_union() && field.is_nullable {
                    reporter.error(
                        virtpath,
                        &obj.fqname,
                        "Nullable fields on unions are not supported.",
                    );
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
                    if field.is_transparent()
                        && let Some(target_fqname) = field.typ.fqname()
                    {
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
                            field
                                .attrs
                                .0
                                .insert(crate::ATTR_TRANSPARENT.to_owned(), transparency.clone());
                        } else {
                            field.attrs.0.remove(crate::ATTR_TRANSPARENT);
                        }

                        done = false;
                    }
                }
            }
        }

        // Remove whole objects marked as transparent.
        this.objects.retain(|_, obj| !obj.is_transparent());

        this
    }
}

/// Ensure that each field of an archetype has exactly one of the
/// `attr.rerun.component_{required|recommended|optional}` attributes.
fn validate_archetype_field_attributes(reporter: &Reporter, obj: &Object) {
    assert_eq!(obj.kind, ObjectKind::Archetype);

    for field in &obj.fields {
        if [
            ATTR_RERUN_COMPONENT_OPTIONAL,
            ATTR_RERUN_COMPONENT_RECOMMENDED,
            ATTR_RERUN_COMPONENT_REQUIRED,
        ]
        .iter()
        .filter(|attr| field.try_get_attr::<String>(attr).is_some())
        .count()
            != 1
        {
            reporter.error(
                &field.virtpath,
                &field.fqname,
                "field must have exactly one of the \"attr.rerun.component_{{required|recommended|\
                optional}}\" attributes",
            );
        }
    }
}

impl Objects {
    pub fn get(&self, fqname: &str) -> Option<&Object> {
        self.objects.get(fqname)
    }

    pub fn values(&self) -> impl Iterator<Item = &Object> {
        self.objects.values()
    }

    /// Returns all available objects of the given kind.
    pub fn objects_of_kind(&self, kind: ObjectKind) -> impl Iterator<Item = &Object> {
        self.objects.values().filter(move |obj| obj.kind == kind)
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

    /// Views are neither archetypes nor components but are used to generate code to make it easy
    /// to add and configure views on the blueprint.
    View,
}

/// Must be set on all archetypes and components
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Used for types that are likely to be removed or changed significantly,
    /// and in a way that the data won't be backwards compatible.
    Unstable,

    /// Used for types that are unlikely to be removed or changed significantly.
    /// If they are changed, we will make sure that the old data can still be read.
    Stable,

    /// Marks something as deprecated followed by a (mandatory!) migration note.
    ///
    /// If specified on an object (struct/enum/union), it becomes deprecated such
    /// that using the object should emit a warning in all target languages.
    /// Furthermore, documentation will mention that the object is deprecated and display
    /// the specified migration note.
    Deprecated { since: String, notice: String },
}

impl State {
    pub fn from_attrs(attrs: &Attributes) -> Result<Self, String> {
        if let Some(state) = attrs.get_string(ATTR_RERUN_STATE) {
            match state.as_str() {
                "unstable" => Ok(Self::Unstable),
                "stable" => Ok(Self::Stable),
                "deprecated" => {
                    if let (Some(since), Some(notice)) = (
                        attrs.get_string(ATTR_RERUN_DEPRECATED_SINCE),
                        attrs.get_string(ATTR_RERUN_DEPRECATED_NOTICE),
                    ) {
                        Ok(Self::Deprecated {
                            since: since.clone(),
                            notice: notice.clone(),
                        })
                    } else {
                        Err(format!(
                            "Deprecated object must have {ATTR_RERUN_DEPRECATED_SINCE:?} and {ATTR_RERUN_DEPRECATED_NOTICE:?} set"
                        ))
                    }
                }
                unknown => Err(format!("Unknown value for {ATTR_RERUN_STATE:?}: {unknown}")),
            }
        } else {
            Err(format!("Missing attribute {ATTR_RERUN_STATE:?}"))
        }
    }

    /// Add noteworthy information on a single line, if any.
    pub fn docline_summary(&self) -> Option<String> {
        match self {
            Self::Unstable => {
                Some("⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**".to_owned())
            }
            Self::Stable => { None }
            Self::Deprecated { since, notice } => {
                Some(format!("⚠️ **Deprecated since {since}**: {notice}"))
            }
        }
    }
}

impl ObjectKind {
    pub const ALL: [Self; 4] = [Self::Datatype, Self::Component, Self::Archetype, Self::View];

    // TODO(#2364): use an attr instead of the path
    pub fn from_pkg_name(pkg_name: &str, attrs: &Attributes) -> Self {
        assert!(!pkg_name.is_empty(), "Missing package name");

        let scope = match attrs.try_get::<String>(pkg_name, crate::ATTR_RERUN_SCOPE) {
            Some(scope) => format!(".{scope}"),
            None => String::new(),
        };

        let pkg_name = pkg_name.replace(".testing", "");
        if pkg_name.starts_with(format!("rerun{scope}.datatypes").as_str()) {
            Self::Datatype
        } else if pkg_name.starts_with(format!("rerun{scope}.components").as_str()) {
            Self::Component
        } else if pkg_name.starts_with(format!("rerun{scope}.archetytpes").as_str()) {
            // TODO(emilk): this is clearly a typo, but fixing it causes problem. GAH
            Self::Archetype
        } else if pkg_name.starts_with("rerun.blueprint.views") {
            // Not bothering with scope attributes on views since they're always part of the blueprint.
            Self::View
        } else {
            panic!("unknown package {pkg_name:?}");
        }
    }

    pub fn plural_snake_case(&self) -> &'static str {
        match self {
            Self::Datatype => "datatypes",
            Self::Component => "components",
            Self::Archetype => "archetypes",
            Self::View => "views",
        }
    }

    pub fn singular_name(&self) -> &'static str {
        match self {
            Self::Datatype => "Datatype",
            Self::Component => "Component",
            Self::Archetype => "Archetype",
            Self::View => "View",
        }
    }

    pub fn plural_name(&self) -> &'static str {
        match self {
            Self::Datatype => "Datatypes",
            Self::Component => "Components",
            Self::Archetype => "Archetypes",
            Self::View => "Views",
        }
    }
}

pub struct ViewReference {
    /// Typename of the view. Not a fully qualified name, just the name as specified on the attribute.
    pub view_name: String,

    pub explanation: Option<String>,
}

/// A high-level representation of a flatbuffers object, which can be either a struct, a union or
/// an enum.
#[derive(Debug, Clone)]
pub struct Object {
    /// `Utf8Path` of the associated fbs definition in the Flatbuffers hierarchy, e.g. `//rerun/components/point2d.fbs`.
    pub virtpath: String,

    /// Absolute filepath of the associated fbs definition.
    pub filepath: Utf8PathBuf,

    /// Fully-qualified name of the object, e.g. `rerun.components.Position2D`.
    pub fqname: String,

    /// Fully-qualified package name of the object, e.g. `rerun.components`.
    pub pkg_name: String,

    /// `PascalCase` name of the object type, e.g. `Position2D`.
    pub name: String,

    /// The object's multiple layers of documentation.
    pub docs: Docs,

    /// The object's kind: datatype, component or archetype.
    pub kind: ObjectKind,

    /// The object's attributes.
    pub attrs: Attributes,

    /// Experimental, stable, deprecated, …?
    pub state: State,

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
    pub datatype: Option<LazyDatatype>,
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.fqname == other.fqname
    }
}

impl Eq for Object {}

impl Ord for Object {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.fqname.cmp(&other.fqname)
    }
}

impl PartialOrd for Object {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Object {
    /// Resolves a raw [`crate::Object`] into a higher-level representation that can be easily
    /// interpreted and manipulated.
    pub fn from_raw_object(
        reporter: &Reporter,
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

        let docs = Docs::from_raw_docs(reporter, &virtpath, obj.name(), obj.documentation());
        let attrs = Attributes::from_raw_attrs(obj.attributes());
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
                    ObjectField::from_raw_object_field(
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

        Self {
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

        let docs = Docs::from_raw_docs(reporter, &virtpath, enm.name(), enm.documentation());
        let attrs = Attributes::from_raw_attrs(enm.attributes());
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
                        .filter(|utype| utype.base_type() != FbsBaseType::None)
                        .is_some()
            })
            .map(|val| {
                ObjectField::from_raw_enum_value(reporter, include_dir_path, enums, objs, enm, &val)
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

        Self {
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

    pub fn archetype_view_types(&self) -> Option<Vec<ViewReference>> {
        let view_types = self.try_get_attr::<String>(crate::ATTR_DOCS_VIEW_TYPES)?;

        Some(
            view_types
                .split(',')
                .map(|view_type| {
                    let mut parts = view_type.splitn(2, ':');
                    let view_name = parts.next().unwrap().trim().to_owned();
                    let explanation = parts.next().map(|s| s.trim().to_owned());
                    ViewReference {
                        view_name,
                        explanation,
                    }
                })
                .collect(),
        )
    }

    pub fn is_struct(&self) -> bool {
        self.class == ObjectClass::Struct
    }

    pub fn is_enum(&self) -> bool {
        self.class.is_enum()
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
        re_case::to_snake_case(&self.name)
    }

    /// Returns true if this object is part of testing and not to be used in the production SDK.
    pub fn is_testing(&self) -> bool {
        is_testing_fqname(&self.fqname)
    }

    /// e.g. `blueprint`
    pub fn scope(&self) -> Option<String> {
        self.try_get_attr::<String>(crate::ATTR_RERUN_SCOPE)
            .or_else(|| (self.kind == ObjectKind::View).then(|| "blueprint".to_owned()))
    }

    pub fn is_deprecated(&self) -> bool {
        matches!(self.state, State::Deprecated { .. })
    }

    /// If deprecated, returns a string describing since when, and what to do instead.
    pub fn deprecation_summary(&self) -> Option<String> {
        if let State::Deprecated { since, notice } = &self.state {
            Some(format!("since {since}: {notice}"))
        } else {
            None
        }
    }

    pub fn doc_category(&self) -> Option<String> {
        self.try_get_attr::<String>(crate::ATTR_DOCS_CATEGORY)
    }

    /// Returns the crate name of an object, accounting for overrides.
    pub fn crate_name(&self) -> String {
        self.try_get_attr::<String>(crate::ATTR_RUST_OVERRIDE_CRATE)
            .unwrap_or_else(|| "re_sdk_types".to_owned())
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

    pub fn is_archetype(&self) -> bool {
        self.kind == ObjectKind::Archetype
    }

    pub fn enum_integer_type(&self) -> Option<EnumIntegerType> {
        match self.class {
            ObjectClass::Enum(enum_type) => Some(enum_type),
            _ => None,
        }
    }
}

pub fn is_testing_fqname(fqname: &str) -> bool {
    fqname.contains("rerun.testing")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnumIntegerType {
    U8,
    U16,
    U32,
    U64,
}

impl EnumIntegerType {
    pub fn to_type(self) -> Type {
        match self {
            Self::U8 => Type::UInt8,
            Self::U16 => Type::UInt16,
            Self::U32 => Type::UInt32,
            Self::U64 => Type::UInt64,
        }
    }

    pub fn format_value(&self, value: u64) -> String {
        match self {
            Self::U8 => format!("{value}"),
            Self::U16 => format!("0x{:0X}", value as u16),
            Self::U32 => format!("0x{:0X}", value as u32),
            Self::U64 => format!("0x{value:0X}"),
        }
    }
}

/// Is this a struct, enum, or union?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectClass {
    Struct,

    /// Dumb C-style enum.
    ///
    /// Encoded as a primitive integer arrow array.
    ///
    /// We reserve `0` for a special/implicit `__null_markers` variant,
    Enum(EnumIntegerType),

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

impl ObjectClass {
    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum(_))
    }
}

/// A high-level representation of a flatbuffers field, which can be either a struct member or a
/// union value.
#[derive(Debug, Clone)]
pub struct ObjectField {
    /// `Utf8Path` of the associated fbs definition in the Flatbuffers hierarchy, e.g. `//rerun/components/point2d.fbs`.
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

    /// The value of the variant for enums & unions.
    pub enum_or_union_variant_value: Option<u64>,

    /// The field's multiple layers of documentation.
    pub docs: Docs,

    /// Experimental, stable, deprecated, …?
    pub state: State,

    /// The field's type.
    pub typ: Type,

    /// The field's attributes.
    pub attrs: Attributes,

    /// The struct field's `order` attribute's value, which is mandatory for struct fields
    /// (otherwise their order is undefined).
    pub order: u32,

    /// Whether the field is nullable.
    pub is_nullable: bool,

    /// The Arrow datatype of this `ObjectField`.
    ///
    /// This is lazily computed when the parent object gets registered into the Arrow registry and
    /// will be `None` until then.
    pub datatype: Option<LazyDatatype>,
}

impl ObjectField {
    pub fn from_raw_object_field(
        reporter: &Reporter,
        include_dir_path: impl AsRef<Utf8Path>,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        obj: &FbsObject<'_>,
        field: &FbsField<'_>,
    ) -> Self {
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

        let docs = Docs::from_raw_docs(reporter, &virtpath, field.name(), field.documentation());

        let attrs = Attributes::from_raw_attrs(field.attributes());
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

        let typ = Type::from_raw_type(&virtpath, enums, objs, field.type_(), &attrs, &fqname);
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

        let enum_value = None;

        Self {
            virtpath,
            filepath,
            fqname,
            pkg_name,
            name,
            enum_or_union_variant_value: enum_value,
            docs,
            state,
            typ,
            attrs,
            order,
            is_nullable,
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

        let docs = Docs::from_raw_docs(reporter, &virtpath, val.name(), val.documentation());

        let attrs = Attributes::from_raw_attrs(val.attributes());
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
        let typ = Type::from_raw_type(&virtpath, enums, objs, field_type, &attrs, &fqname);

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

        let enum_value = Some(val.value() as u64);

        Self {
            virtpath,
            filepath,
            fqname,
            pkg_name,
            name,
            enum_or_union_variant_value: enum_value,
            state,
            docs,
            typ,
            attrs,
            order: 0, // no needed for enums
            is_nullable,
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
        re_case::to_snake_case(&self.name)
    }

    /// The `PascalCase` name of the field, e.g. `TranslationAndMat3x3`.
    pub fn pascal_case_name(&self) -> String {
        re_case::to_pascal_case(&self.name)
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

    pub fn make_plural(&self) -> Option<Self> {
        self.typ.make_plural().map(|typ| Self {
            typ,
            ..self.clone()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Required,
    Recommended,
    Optional,
}

impl FieldKind {
    pub const ALL: [Self; 3] = [Self::Required, Self::Recommended, Self::Optional];
}

impl std::fmt::Display for FieldKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Required => "Required".fmt(f),
            Self::Recommended => "Recommended".fmt(f),
            Self::Optional => "Optional".fmt(f),
        }
    }
}

/// The underlying type of an [`ObjectField`].
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    /// This is the unit type, used for `enum` variants.
    ///
    /// In `arrow`, this corresponds to the `null` type.
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

    /// A list of bytes of arbitrary length.
    ///
    /// 32-bit or 64-bit
    Binary,

    /// Utf8
    String,

    Array {
        elem_type: ElementType,
        length: usize,
    },
    Vector {
        elem_type: ElementType,
    },
    Object {
        fqname: String,
    },
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
            ElementType::Binary => Self::Binary,
            ElementType::String => Self::String,
            ElementType::Object { fqname } => Self::Object { fqname },
        }
    }
}

impl Type {
    pub fn from_raw_type(
        virtpath: &str,
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        field_type: FbsType<'_>,
        attrs: &Attributes,
        fqname: &str,
    ) -> Self {
        let typ = field_type.base_type();

        if let Some(type_override) = attrs.try_get::<String>(fqname, ATTR_RERUN_OVERRIDE_TYPE) {
            match type_override.as_str() {
                "binary" => {
                    if typ == FbsBaseType::Vector && field_type.element() == FbsBaseType::UByte {
                        return Self::Binary;
                    } else {
                        panic!("{fqname}: 'binary' can only be used on '[ubyte]', got {typ:?}")
                    }
                }
                "float16" => {
                    if matches!(typ, FbsBaseType::Array | FbsBaseType::Vector) {
                        // Array of float16 handled later
                    } else if typ == FbsBaseType::UShort {
                        return Self::Float16;
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
            return Self::Object {
                fqname: enum_fqname,
            };
        }

        match typ {
            FbsBaseType::None => Self::Unit, // Enum variant

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
                if obj.name() == BUILTIN_UNIT_TYPE_FQNAME {
                    Self::Unit
                } else {
                    Self::Object {
                        fqname: obj.name().to_owned(),
                    }
                }
            }
            FbsBaseType::Union => {
                let union = &enums[field_type.index() as usize];
                Self::Object {
                    fqname: union.name().to_owned(),
                }
            }
            FbsBaseType::Array => Self::Array {
                elem_type: ElementType::from_raw_base_type(
                    enums,
                    objs,
                    field_type,
                    field_type.element(),
                    attrs,
                    virtpath,
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

    pub fn make_plural(&self) -> Option<Self> {
        match self {
            Self::Vector { elem_type: _ }
            | Self::Array {
                elem_type: _,
                length: _,
            } => Some(self.clone()),

            Self::UInt8 => Some(Self::Vector {
                elem_type: ElementType::UInt8,
            }),
            Self::UInt16 => Some(Self::Vector {
                elem_type: ElementType::UInt16,
            }),
            Self::UInt32 => Some(Self::Vector {
                elem_type: ElementType::UInt32,
            }),
            Self::UInt64 => Some(Self::Vector {
                elem_type: ElementType::UInt64,
            }),
            Self::Int8 => Some(Self::Vector {
                elem_type: ElementType::Int8,
            }),
            Self::Int16 => Some(Self::Vector {
                elem_type: ElementType::Int16,
            }),
            Self::Int32 => Some(Self::Vector {
                elem_type: ElementType::Int32,
            }),
            Self::Int64 => Some(Self::Vector {
                elem_type: ElementType::Int64,
            }),
            Self::Bool => Some(Self::Vector {
                elem_type: ElementType::Bool,
            }),
            Self::Float16 => Some(Self::Vector {
                elem_type: ElementType::Float16,
            }),
            Self::Float32 => Some(Self::Vector {
                elem_type: ElementType::Float32,
            }),
            Self::Float64 => Some(Self::Vector {
                elem_type: ElementType::Float64,
            }),
            Self::Binary => Some(Self::Vector {
                elem_type: ElementType::Binary,
            }),
            Self::String => Some(Self::Vector {
                elem_type: ElementType::String,
            }),
            Self::Object { fqname } => Some(Self::Vector {
                elem_type: ElementType::Object {
                    fqname: fqname.clone(),
                },
            }),

            Self::Unit => None,
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
            | Self::Binary
            | Self::String
            | Self::Object { .. } => None,
        }
    }

    pub fn vector_inner(&self) -> Option<&ElementType> {
        self.plural_inner()
            .filter(|_| matches!(self, Self::Vector { .. }))
    }

    /// `Some(fqname)` if this is an `Object` or an `Array`/`Vector` of `Object`s.
    pub fn fqname(&self) -> Option<&str> {
        match self {
            Self::Object { fqname } => Some(fqname.as_str()),
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

            Self::Binary | Self::String | Self::Vector { .. } => false,

            Self::Array { elem_type, .. } => elem_type.has_default_destructor(objects),

            Self::Object { fqname } => objects[fqname].has_default_destructor(objects),
        }
    }

    pub fn is_union(&self, objects: &Objects) -> bool {
        if let Self::Object { fqname } = self {
            let obj = &objects[fqname];
            if obj.is_arrow_transparent() {
                obj.fields[0].typ.is_union(objects)
            } else {
                obj.class == ObjectClass::Union
            }
        } else {
            false
        }
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

    /// A list of bytes of arbitrary length.
    ///
    /// 32-bit or 64-bit
    Binary,

    /// Utf8
    String,

    Object {
        fqname: String,
    },
}

impl ElementType {
    pub fn from_raw_base_type(
        enums: &[FbsEnum<'_>],
        objs: &[FbsObject<'_>],
        outer_type: FbsType<'_>,
        inner_type: FbsBaseType,
        attrs: &Attributes,
        virtpath: &str,
    ) -> Self {
        if let Some(enum_fqname) = try_get_enum_fqname(enums, outer_type, inner_type, virtpath) {
            return Self::Object {
                fqname: enum_fqname,
            };
        }

        // TODO(jleibs): Clean up fqname plumbing
        let fqname = "???";
        if let Some(type_override) = attrs.try_get::<String>(fqname, ATTR_RERUN_OVERRIDE_TYPE) {
            match (inner_type, type_override.as_str()) {
                (FbsBaseType::UShort, "float16") => {
                    return Self::Float16;
                }
                _ => unreachable!(
                    "UShort -> float16 is the only permitted type override. Not {inner_type:#?}->{type_override}"
                ),
            }
        }

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
                Self::Object {
                    fqname: obj.name().to_owned(),
                }
            }
            FbsBaseType::Union => {
                let enm = &enums[outer_type.index() as usize];
                Self::Object {
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

    /// `Some(fqname)` if this is an `Object`.
    pub fn fqname(&self) -> Option<&str> {
        match self {
            Self::Object { fqname } => Some(fqname.as_str()),
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

            Self::Binary | Self::String => false,

            Self::Object { fqname } => objects[fqname].has_default_destructor(objects),
        }
    }

    /// Is this type directly backed by a native arrow `Buffer`. This means the data can
    /// be returned using a `ScalarBuffer` which facilitates direct zero-copy access to
    /// a slice representation.
    pub fn backed_by_scalar_buffer(&self) -> bool {
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
            Self::Bool | Self::Binary | Self::String | Self::Object { .. } => false,
        }
    }

    pub fn is_union(&self, objects: &Objects) -> bool {
        if let Self::Object { fqname } = self {
            let obj = &objects[fqname];
            if obj.is_arrow_transparent() {
                obj.fields[0].typ.is_union(objects)
            } else {
                obj.class == ObjectClass::Union
            }
        } else {
            false
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

    pub fn get_string(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned().flatten()
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

    pub fn remove(&mut self, name: impl AsRef<str>) {
        self.0.remove(name.as_ref());
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
