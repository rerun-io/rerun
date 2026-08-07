//! This package implements the semantic pass of the codegen process.
//!
//! The semantic pass transforms the low-level raw reflection data into higher level types that
//! are much easier to inspect and manipulate / friendlier to work with.
//!
//! Everything in here is IDL-agnostic: it is a plain intermediate representation with no notion
//! of the syntax it was parsed from.
//! The frontend that produces it lives in [`from_rust`].

pub(crate) mod from_rust;

use std::collections::BTreeMap;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};

use crate::data_type::LazyDatatype;
use crate::{Docs, Reporter, RerunAttr};

// ---

/// The result of the semantic pass: an intermediate representation of all available object
/// types; including structs, enums and unions.
#[derive(Debug, Default)]
pub struct Objects {
    /// Maps fully-qualified type names to their resolved object definitions.
    pub objects: BTreeMap<String, Object>,
}

impl Objects {
    /// The IDL-agnostic half of the semantic pass.
    ///
    /// Validates the object graph. Every frontend must call this once it has produced the raw
    /// [`Object`] map.
    pub(crate) fn validate(&self, reporter: &Reporter) {
        // Validate field types: archetypes consist of components, Views (aka SuperArchetypes) consist of archetypes, everything else consists of datatypes.
        for obj in self.objects.values() {
            for field in &obj.fields {
                let virtpath = &field.virtpath;
                if let Some(field_type_fqname) = field.typ.fqname() {
                    let field_obj = &self[field_type_fqname];
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

                // Validate whether someone is using a type we use for non-nullable arrays to describe some nullable field.
                if field.is_nullable
                    && (obj.kind == ObjectKind::Datatype || obj.kind == ObjectKind::Component)
                    && let Some(field_type_fqname) = field.typ.fqname()
                    // TODO(andreas): This is a hack, here because introducing this warning, I really don't want to touch annotation info right now.
                    && obj.name != "AnnotationInfo"
                {
                    let field_obj = &self[field_type_fqname];
                    if field_obj.is_arrow_transparent() {
                        let suggestion = if field_obj.name == "Utf8" {
                            "Use `string (nullable)` instead of `rerun.datatypes.Utf8 (nullable)`."
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
    pub const ALL: [Self; 4] = [Self::Datatype, Self::Component, Self::Archetype, Self::View];

    // TODO(#2364): use an attr instead of the path
    pub fn from_pkg_name(pkg_name: &str, attrs: &Attributes) -> Self {
        assert!(!pkg_name.is_empty(), "Missing package name");

        let scope = match attrs.try_get::<String>(pkg_name, crate::RerunAttr::Scope) {
            Some(scope) => format!(".{scope}"),
            None => String::new(),
        };

        let pkg_name = pkg_name.replace(".testing", "");
        if pkg_name.starts_with(format!("rerun{scope}.datatypes").as_str()) {
            Self::Datatype
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

    /// The object's kind: datatype, component or archetype.
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

    /// Returns the suffix used for the repr type, e.g. `"u8"`, `"u16"`, etc.
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

    /// The Arrow datatype of this `ObjectField`.
    ///
    /// This is lazily computed when the parent object gets registered into the Arrow registry and
    /// will be `None` until then.
    pub datatype: Option<LazyDatatype>,
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
            ElementType::Array { elem_type, length } => Self::Array {
                elem_type: *elem_type,
                length,
            },
        }
    }
}

impl Type {
    /// The inverse of `From<ElementType> for Type`: the element type that this type
    /// corresponds to when used as an array/vector element.
    ///
    /// Returns `None` for types that cannot be element types (`Unit`, vectors).
    pub fn to_element_type(&self) -> Option<ElementType> {
        match self {
            Self::UInt8 => Some(ElementType::UInt8),
            Self::UInt16 => Some(ElementType::UInt16),
            Self::UInt32 => Some(ElementType::UInt32),
            Self::UInt64 => Some(ElementType::UInt64),
            Self::Int8 => Some(ElementType::Int8),
            Self::Int16 => Some(ElementType::Int16),
            Self::Int32 => Some(ElementType::Int32),
            Self::Int64 => Some(ElementType::Int64),
            Self::Bool => Some(ElementType::Bool),
            Self::Float16 => Some(ElementType::Float16),
            Self::Float32 => Some(ElementType::Float32),
            Self::Float64 => Some(ElementType::Float64),
            Self::Binary => Some(ElementType::Binary),
            Self::String => Some(ElementType::String),
            Self::Object { fqname } => Some(ElementType::Object {
                fqname: fqname.clone(),
            }),
            Self::Array { elem_type, length } => Some(ElementType::Array {
                elem_type: Box::new(elem_type.clone()),
                length: *length,
            }),

            Self::Unit | Self::Vector { .. } => None,
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

    /// A nested fixed-size array.
    ///
    /// This cannot be expressed directly in the definitions (arrays cannot nest);
    /// it is produced by the semantic pass when a `transparent` struct wrapping a
    /// fixed-size array is used as the element type of an array/vector.
    Array {
        elem_type: Box<Self>,
        length: usize,
    },
}

impl ElementType {
    /// `Some(fqname)` if this is an `Object`.
    pub fn fqname(&self) -> Option<&str> {
        match self {
            Self::Object { fqname } => Some(fqname.as_str()),
            _ => None,
        }
    }

    /// Recursively resolves nested arrays to their innermost element type.
    ///
    /// Returns `self` for everything but [`Self::Array`].
    pub fn innermost_element_type(&self) -> &Self {
        match self {
            Self::Array { elem_type, .. } => elem_type.innermost_element_type(),
            _ => self,
        }
    }

    /// `Some(Object)` if this is an enum object.
    pub fn enum_obj<'a>(&self, objects: &'a Objects) -> Option<&'a Object> {
        let Self::Object { fqname } = self else {
            return None;
        };

        let obj = &objects[fqname];
        obj.is_enum().then_some(obj)
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

            Self::Array { elem_type, .. } => elem_type.has_default_destructor(objects),
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
            Self::Bool | Self::Binary | Self::String | Self::Object { .. } | Self::Array { .. } => {
                false
            }
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
