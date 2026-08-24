//! The intermediate representation the whole pipeline is written against, and its validation.
//!
//! Everything in here is IDL-agnostic: it is a plain intermediate representation with no notion
//! of the syntax it was parsed from.
//! The frontend that produces it lives in [`from_rust`].

pub(crate) mod from_rust;

use std::collections::BTreeMap;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};

use crate::data_type::AtomicDataType;
use crate::{Docs, Reporter, RerunAttr};

// ---

/// An intermediate representation of all available object types; including structs, enums and
/// unions.
#[derive(Debug, Default)]
pub struct Objects {
    /// Maps fully-qualified type names to their resolved object definitions.
    pub objects: BTreeMap<String, Object>,
}

impl Objects {
    /// Validates the object graph.
    ///
    /// Every frontend must call this once it has produced the raw [`Object`] map.
    pub(crate) fn validate(&self, reporter: &Reporter) {
        // Validate field types: archetypes consist of components, Views (aka SuperArchetypes) consist of archetypes, everything else consists of encodings.
        for obj in self.objects.values() {
            for field in &obj.fields {
                let virtpath = &field.virtpath;
                if let Some(field_type_fqname) = field.typ.fqname() {
                    let field_obj = &self[field_type_fqname];
                    match obj.kind {
                        ObjectKind::Encoding | ObjectKind::Component => {
                            if field_obj.kind != ObjectKind::Encoding {
                                reporter.error(virtpath, field_type_fqname, "Is part of a Component or Encoding but is itself not an Encoding. Only archetype fields can be components, all other fields have to be primitive or be an encoding.");
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
                } else if obj.kind != ObjectKind::Encoding {
                    let is_enum_component = obj.kind == ObjectKind::Component && obj.is_enum(); // Enum components are allowed to have no datatype.
                    let is_test_component = obj.kind == ObjectKind::Component && obj.is_testing(); // Test components are allowed to have encodings for the moment. TODO(andreas): Should clean this up as well!
                    if !is_enum_component && !is_test_component {
                        reporter.error(virtpath, &obj.fqname, format!("Field {:?} s a primitive field of type {:?}. Primitive types are only allowed on encodings.", field.fqname, field.typ));
                    }
                }

                if obj.is_union() && field.is_nullable {
                    reporter.error(
                        virtpath,
                        &obj.fqname,
                        "Nullable fields on unions are not supported.",
                    );
                }

                // Validate whether someone is using a type we use for non-nullable arrays to describe some nullable field.
                if field.is_nullable
                    && (obj.kind == ObjectKind::Encoding || obj.kind == ObjectKind::Component)
                    && let Some(field_type_fqname) = field.typ.fqname()
                    // TODO(andreas): This is a hack, here because introducing this warning, I really don't want to touch annotation info right now.
                    && obj.name != "AnnotationInfo"
                {
                    let field_obj = &self[field_type_fqname];
                    if field_obj.is_arrow_transparent() {
                        let suggestion = if field_obj.name == "Utf8" {
                            "Use `string (nullable)` instead of `rerun.encodings.Utf8 (nullable)`."
                                .to_owned()
                        } else {
                            format!(
                                "Consider using a primitive type instead of nullable transparent wrapper `{}`.",
                                field_obj.name
                            )
                        };

                        reporter.warn(
                                virtpath,
                                field_type_fqname,
                                format!(
                                    "Nullable transparent wrapper type detected. {} \
                                     Transparent wrapper types like '{}' don't handle None internally, \
                                     which can cause serialization issues.",
                                    suggestion,
                                    field_obj.name
                                ),
                            );
                    }
                }
            }
        }
    }
}

/// Ensure that each field of an archetype belongs to exactly one of the three component lists.
fn validate_archetype_field_attributes(reporter: &Reporter, obj: &Object) {
    assert_eq!(obj.kind, ObjectKind::Archetype);

    const LISTS: [RerunAttr; 3] = [
        RerunAttr::Optional,
        RerunAttr::Recommended,
        RerunAttr::Required,
    ];

    for field in &obj.fields {
        if LISTS.iter().filter(|attr| field.has_attr(attr)).count() != 1 {
            reporter.error(
                &field.virtpath,
                &field.fqname,
                format!(
                    "field must have exactly one of the `{}`, `{}` and `{}` attributes",
                    RerunAttr::Optional,
                    RerunAttr::Recommended,
                    RerunAttr::Required
                ),
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
/// let obj = &objects["rerun.encodings.Vec3D"];
/// let obj = &objects["rerun.encodings.Angle"];
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
    Encoding,
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
        if let Some(state) = attrs.get_string(RerunAttr::State) {
            match state.as_str() {
                "unstable" => Ok(Self::Unstable),
                "stable" => Ok(Self::Stable),
                "deprecated" => {
                    if let (Some(since), Some(notice)) = (
                        attrs.get_string(RerunAttr::DeprecatedSince),
                        attrs.get_string(RerunAttr::DeprecatedNotice),
                    ) {
                        Ok(Self::Deprecated {
                            since: since.clone(),
                            notice: notice.clone(),
                        })
                    } else {
                        Err(format!(
                            "Deprecated object must have `{}` and `{}` set",
                            RerunAttr::DeprecatedSince,
                            RerunAttr::DeprecatedNotice
                        ))
                    }
                }
                unknown => Err(format!(
                    "Unknown value for `{}`: {unknown}",
                    RerunAttr::State
                )),
            }
        } else {
            Err(format!("Missing attribute `{}`", RerunAttr::State))
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
    pub const ALL: [Self; 4] = [Self::Encoding, Self::Component, Self::Archetype, Self::View];

    // TODO(#2364): use an attr instead of the path
    pub fn from_pkg_name(pkg_name: &str, attrs: &Attributes) -> Self {
        assert!(!pkg_name.is_empty(), "Missing package name");

        let scope = match attrs.try_get::<String>(pkg_name, crate::RerunAttr::Scope) {
            Some(scope) => format!(".{scope}"),
            None => String::new(),
        };

        let pkg_name = pkg_name.replace(".testing", "");
        if pkg_name.starts_with(format!("rerun{scope}.encodings").as_str()) {
            Self::Encoding
        } else if pkg_name.starts_with(format!("rerun{scope}.components").as_str()) {
            Self::Component
        } else if pkg_name.starts_with(format!("rerun{scope}.archetypes").as_str()) {
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
            Self::Encoding => "encodings",
            Self::Component => "components",
            Self::Archetype => "archetypes",
            Self::View => "views",
        }
    }

    pub fn singular_name(&self) -> &'static str {
        match self {
            Self::Encoding => "Encoding",
            Self::Component => "Component",
            Self::Archetype => "Archetype",
            Self::View => "View",
        }
    }

    pub fn plural_name(&self) -> &'static str {
        match self {
            Self::Encoding => "Encodings",
            Self::Component => "Components",
            Self::Archetype => "Archetypes",
            Self::View => "Views",
        }
    }

    /// Are the docs pages for this kind still missing from the live site, so that any link to
    /// one of them would 404?
    ///
    /// True for [`Self::Encoding`] until 0.37: `datatypes` was renamed to `encodings`, which
    /// moves every `reference/types/encodings/…` page, and every `encodings` anchor in the
    /// per-language API docs, to a URL that is only published when 0.37 is released.
    ///
    /// TODO(RR-5430): remove once 0.37 is released.
    pub fn has_unpublished_docs(&self) -> bool {
        match self {
            Self::Encoding => {
                // E.g. `CARGO_PKG_VERSION` = "0.37.0-alpha.1+dev",
                // `CARGO_PKG_VERSION_MINOR` = "37", `CARGO_PKG_VERSION_PRE` = "alpha.1".
                // A non-empty pre-release means 0.37 is still cooking.
                let Ok(minor) = env!("CARGO_PKG_VERSION_MINOR").parse::<u32>() else {
                    // Assume published rather than littering every link with a marker.
                    return false;
                };
                let is_pre_release = !env!("CARGO_PKG_VERSION_PRE").is_empty();

                minor < 37 || (minor == 37 && is_pre_release)
            }
            Self::Component | Self::Archetype | Self::View => false,
        }
    }
}

pub struct ViewReference {
    /// Typename of the view. Not a fully qualified name, just the name as specified on the attribute.
    pub view_name: String,

    pub explanation: Option<String>,
}

/// A high-level representation of a type definition, which can be either a struct, a union or
/// an enum.
#[derive(Debug, Clone)]
pub struct Object {
    /// `Utf8Path` of the definition, relative to the definitions root, e.g.
    /// `//rerun/components/point2d.def.rs`.
    pub virtpath: String,

    /// Absolute filepath of the definition.
    pub filepath: Utf8PathBuf,

    /// Fully-qualified name of the object, e.g. `rerun.components.Position2D`.
    pub fqname: String,

    /// Fully-qualified package name of the object, e.g. `rerun.components`.
    pub pkg_name: String,

    /// `PascalCase` name of the object type, e.g. `Position2D`.
    pub name: String,

    /// The object's multiple layers of documentation.
    pub docs: Docs,

    /// The object's kind: encoding, component or archetype.
    pub kind: ObjectKind,

    /// The object's attributes.
    pub attrs: Attributes,

    /// Experimental, stable, deprecated, …?
    pub state: State,

    /// The object's inner fields, which can be either struct members or union/emum variants.
    ///
    /// These are in source order (structs),
    /// or in the same order that they appeared in the definition (enum/union).
    pub fields: Vec<ObjectField>,

    /// struct, enum, or union?
    pub class: ObjectClass,
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
        let view_types = self.try_get_attr::<String>(crate::DocsAttr::ViewTypes)?;

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
        self.kind == ObjectKind::Component || self.attrs.has(crate::ArrowAttr::Transparent)
    }

    /// Is the destructor trivial/default (i.e. is this simple data with no allocations)?
    pub fn has_default_destructor(&self, objects: &Objects) -> bool {
        self.fields
            .iter()
            .all(|field| field.typ.has_default_destructor(objects))
    }

    /// Try to find the relative file path of the definition.
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
        self.try_get_attr::<String>(crate::RerunAttr::Scope)
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
        self.try_get_attr::<String>(crate::DocsAttr::Category)
    }

    /// Returns the crate name of an object, accounting for overrides.
    pub fn crate_name(&self) -> String {
        self.try_get_attr::<String>(crate::RustAttr::OverrideCrate)
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

/// What integer a C-style `enum` is stored as, i.e. the `#[repr(u8)]` of its definition.
///
/// Unsigned only, and it is the arrow datatype too ([`Self::to_atomic`]): an enum is a plain
/// integer array, not a union.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnumIntegerType {
    U8,
    U16,
    U32,
    U64,
}

impl EnumIntegerType {
    pub fn to_atomic(self) -> AtomicDataType {
        match self {
            Self::U8 => AtomicDataType::UInt8,
            Self::U16 => AtomicDataType::UInt16,
            Self::U32 => AtomicDataType::UInt32,
            Self::U64 => AtomicDataType::UInt64,
        }
    }

    pub fn to_type(self) -> Type {
        Type::Atomic(self.to_atomic())
    }

    pub fn format_value(&self, value: u64) -> String {
        match self {
            Self::U8 => format!("{value}"),
            Self::U16 => format!("0x{:0X}", value as u16),
            Self::U32 => format!("0x{:0X}", value as u32),
            Self::U64 => format!("0x{value:0X}"),
        }
    }

    /// The Rust spelling of the repr type, e.g. `"u8"`.
    pub fn type_str(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }
}

/// Is this a struct, enum, or union?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectClass {
    Struct,

    /// Dumb C-style enum, whose variants carry no payload.
    ///
    /// Encoded as a primitive integer arrow array, of the given [`EnumIntegerType`].
    /// We reserve `0` as the invalid value, so that a zeroed byte never names a real variant.
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

/// A high-level representation of a field, which can be either a struct member or a
/// union value.
#[derive(Debug, Clone)]
pub struct ObjectField {
    /// `Utf8Path` of the definition, relative to the definitions root, e.g.
    /// `//rerun/components/point2d.def.rs`.
    pub virtpath: String,

    /// Absolute filepath of the definition.
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

    /// Whether the field is nullable.
    pub is_nullable: bool,
}

impl ObjectField {
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
        if self.has_attr(crate::RerunAttr::Required) {
            Some(FieldKind::Required)
        } else if self.has_attr(crate::RerunAttr::Recommended) {
            Some(FieldKind::Recommended)
        } else if self.has_attr(crate::RerunAttr::Optional) {
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

/// The type of an [`ObjectField`], as the definition wrote it.
///
/// This is the definition half of the type system, so the variants are the things a definition may
/// spell: `f32`, `String`, `Vec<f32>`, `[f32; 3]`, `rerun::datatypes::Vec3D`. They are named after
/// their arrow counterparts because that is what they become —
/// [`TypeRegistry`](crate::TypeRegistry) derives a [`DataType`](crate::data_type::DataType) from
/// each one — but they are not the same thing: a `Type` says what the author asked for, a
/// `DataType` says how it is laid out.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    /// A number, a boolean, or the unit type.
    ///
    /// The unit type is [`AtomicDataType::Null`]; see [`Self::is_unit`].
    Atomic(AtomicDataType),

    /// A list of bytes of arbitrary length, i.e. `rerun::Binary`.
    Binary,

    /// A string of arbitrary length, i.e. `String`.
    Utf8,

    /// A fixed-size array, e.g. `[f32; 3]`.
    ///
    /// The element type is never the unit type or a [`Self::List`]:
    /// the frontend rejects both.
    FixedSizeList { elem_type: Box<Self>, length: usize },

    /// A list of arbitrary length, e.g. `Vec<f32>`.
    ///
    /// The element type is never the unit type or a [`Self::List`]:
    /// the frontend rejects both.
    List { elem_type: Box<Self> },

    /// Another definition, referred to by fully-qualified name, e.g. `rerun::datatypes::Vec3D`.
    Object { fqname: String },
}

impl Type {
    /// The unit type, used for `enum` variants.
    ///
    /// In `arrow` this is the `null` type, in Rust it is `()`, and in C++ it is `void`.
    pub const UNIT: Self = Self::Atomic(AtomicDataType::Null);

    /// Is this the unit type, i.e. an `enum` variant with no payload?
    pub fn is_unit(&self) -> bool {
        self == &Self::UNIT
    }

    /// A list of `self`, or `self` if it is already a list or a fixed-size list.
    ///
    /// `None` for the unit type, which cannot be an element type.
    pub fn make_plural(&self) -> Option<Self> {
        if self.is_unit() {
            None // An array of nothing is nothing.
        } else if self.is_plural() {
            Some(self.clone())
        } else {
            Some(Self::List {
                elem_type: Box::new(self.clone()),
            })
        }
    }

    /// True if this is some kind of list.
    pub fn is_plural(&self) -> bool {
        self.plural_inner().is_some()
    }

    /// Returns element type for lists and fixed-size lists.
    pub fn plural_inner(&self) -> Option<&Self> {
        match self {
            Self::List { elem_type }
            | Self::FixedSizeList {
                elem_type,
                length: _,
            } => Some(elem_type),

            Self::Atomic(_) | Self::Binary | Self::Utf8 | Self::Object { .. } => None,
        }
    }

    /// Like [`Self::plural_inner`], but only for [`Self::List`], not for fixed-size ones.
    pub fn list_inner(&self) -> Option<&Self> {
        self.plural_inner()
            .filter(|_| matches!(self, Self::List { .. }))
    }

    /// Recursively resolves nested arrays and lists to their innermost element type.
    ///
    /// Returns `self` for everything but [`Self::FixedSizeList`] and [`Self::List`].
    pub fn innermost_element_type(&self) -> &Self {
        match self {
            Self::FixedSizeList { elem_type, .. } | Self::List { elem_type } => {
                elem_type.innermost_element_type()
            }
            _ => self,
        }
    }

    /// `Some(Object)` if this is an enum object.
    pub fn enum_obj<'a>(&self, objects: &'a Objects) -> Option<&'a Object> {
        match self {
            Self::Object { fqname } => enum_obj_of(objects, fqname),
            _ => None,
        }
    }

    /// Is this type directly backed by a native arrow `Buffer`. This means the data can
    /// be returned using a `ScalarBuffer` which facilitates direct zero-copy access to
    /// a slice representation.
    pub fn backed_by_scalar_buffer(&self) -> bool {
        match self {
            Self::Atomic(atomic) => atomic.backed_by_scalar_buffer(),
            _ => false,
        }
    }

    /// `Some(fqname)` if this is an `Object`, or a (possibly nested) list of `Object`s.
    pub fn fqname(&self) -> Option<&str> {
        match self {
            Self::Object { fqname } => Some(fqname.as_str()),
            Self::FixedSizeList {
                elem_type,
                length: _,
            }
            | Self::List { elem_type } => elem_type.fqname(),
            _ => None,
        }
    }

    /// Is the destructor trivial/default (i.e. is this simple data with no allocations)?
    pub fn has_default_destructor(&self, objects: &Objects) -> bool {
        match self {
            Self::Atomic(_) => true,

            Self::Binary | Self::Utf8 | Self::List { .. } => false,

            Self::FixedSizeList { elem_type, .. } => elem_type.has_default_destructor(objects),

            Self::Object { fqname } => objects[fqname].has_default_destructor(objects),
        }
    }

    pub fn is_union(&self, objects: &Objects) -> bool {
        match self {
            Self::Object { fqname } => is_union_fqname(objects, fqname),
            _ => false,
        }
    }
}

/// `Some(Object)` if `fqname` names an enum.
pub(crate) fn enum_obj_of<'a>(objects: &'a Objects, fqname: &str) -> Option<&'a Object> {
    let obj = &objects[fqname];
    obj.is_enum().then_some(obj)
}

/// Does `fqname` name a union, looking through arrow-transparent objects?
fn is_union_fqname(objects: &Objects, fqname: &str) -> bool {
    let obj = &objects[fqname];
    if obj.is_arrow_transparent() {
        obj.fields[0].typ.is_union(objects)
    } else {
        obj.class == ObjectClass::Union
    }
}

// --- Common ---

/// A collection of arbitrary attributes.
#[derive(Debug, Default, Clone)]
pub struct Attributes(BTreeMap<String, Option<String>>);

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

    pub fn get_string(&self, name: impl AsRef<str>) -> Option<String> {
        self.0.get(name.as_ref()).cloned().flatten()
    }

    /// Every attribute, sorted by name, with its value if it has one.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, Option<&str>)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_deref()))
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
