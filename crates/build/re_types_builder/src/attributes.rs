//! Every attribute a type definition can carry.
//!
//! An attribute is written `#[rust(derive = "Default")]` in a definition and stored as
//! `attr.rust.derive` in an [`Attributes`](crate::Attributes) map; see
//! [`objects::from_rust`](crate::objects::from_rust).
//!
//! There is one enum per namespace, and [`Attribute`] over all of them, which closes the set: the
//! frontend rejects anything that is not in here, because an attribute nobody reads is otherwise
//! silently ignored, and a definition that says something nothing acts on is the one way it can
//! mean something other than what it says.
//!
//! The `#[strum(serialize = …)]` on each variant is the name the attribute is stored under, and is
//! what [`AsRef<str>`] and [`Display`](std::fmt::Display) give you, so an attribute can be passed
//! wherever that name is expected.

use strum::{AsRefStr, Display, EnumIter, IntoEnumIterator as _};

/// How a type is laid out in Arrow.
#[derive(AsRefStr, Clone, Copy, Debug, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArrowAttr {
    /// Encode the union as a sparse Arrow union rather than a dense one.
    #[strum(serialize = "attr.arrow.sparse_union")]
    SparseUnion,

    /// The type is stored as its single field, with no struct around it.
    #[strum(serialize = "attr.arrow.transparent")]
    Transparent,
}

/// What the C++ backend does with a type.
#[derive(AsRefStr, Clone, Copy, Debug, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord)]
pub enum CppAttr {
    /// Generate no default constructor; the `_ext.cpp` provides one.
    #[strum(serialize = "attr.cpp.no_default_ctor")]
    NoDefaultCtor,

    /// Generate none of the per-field constructors; the `_ext.cpp` provides them.
    #[strum(serialize = "attr.cpp.no_field_ctors")]
    NoFieldCtors,

    /// Name the field something else in C++, where its own name is taken or reserved.
    #[strum(serialize = "attr.cpp.rename_field")]
    RenameField,
}

/// How a type is presented in the documentation.
#[derive(AsRefStr, Clone, Copy, Debug, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord)]
pub enum DocsAttr {
    /// The heading the type is listed under, e.g. `Spatial 3D`.
    #[strum(serialize = "attr.docs.category")]
    Category,

    /// The type is not in a release yet, and is marked as such wherever it is documented.
    #[strum(serialize = "attr.docs.unreleased")]
    Unreleased,

    /// The views an archetype can be shown in, as `View`, or `View: why`, separated by commas.
    #[strum(serialize = "attr.docs.view_types")]
    ViewTypes,
}

/// What the Python backend does with a type.
#[derive(AsRefStr, Clone, Copy, Debug, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord)]
pub enum PythonAttr {
    /// Extra Python types the type can be built from, as a type annotation.
    #[strum(serialize = "attr.python.aliases")]
    Aliases,

    /// Extra Python types a batch of the type can be built from, as a type annotation.
    #[strum(serialize = "attr.python.array_aliases")]
    ArrayAliases,
}

/// What a type means to Rerun, in every language.
#[derive(AsRefStr, Clone, Copy, Debug, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord)]
pub enum RerunAttr {
    /// What to use instead; required when [`State`](Self::State) is `deprecated`.
    #[strum(serialize = "attr.rerun.deprecated_notice")]
    DeprecatedNotice,

    /// The version it was deprecated in; required when [`State`](Self::State) is `deprecated`.
    #[strum(serialize = "attr.rerun.deprecated_since")]
    DeprecatedSince,

    /// The field is not editable in the viewer's UI.
    #[strum(serialize = "attr.rerun.no_ui_edit")]
    NoUiEdit,

    /// One of the three lists an archetype field belongs to; see also
    /// [`Recommended`](Self::Recommended) and [`Required`](Self::Required).
    #[strum(serialize = "attr.rerun.optional")]
    Optional,

    /// The Arrow type of the field, when it is not the one its Rust type implies.
    #[strum(serialize = "attr.rerun.override_type")]
    OverrideType,

    /// The field has a good default, so it is set unless the user says otherwise; see also
    /// [`Optional`](Self::Optional) and [`Required`](Self::Required).
    #[strum(serialize = "attr.rerun.recommended")]
    Recommended,

    /// The field must be given; see also [`Optional`](Self::Optional) and
    /// [`Recommended`](Self::Recommended).
    #[strum(serialize = "attr.rerun.required")]
    Required,

    /// Which part of Rerun the type belongs to, e.g. `blueprint`. It is part of the package name.
    #[strum(serialize = "attr.rerun.scope")]
    Scope,

    /// How far along the type is: `unstable`, `stable` or `deprecated`.
    #[strum(serialize = "attr.rerun.state")]
    State,

    /// The name a view is registered and addressed by, e.g. `3D`.
    #[strum(serialize = "attr.rerun.view_identifier")]
    ViewIdentifier,

    /// The visualizer that draws the archetype; see also [`VisualizerNone`](Self::VisualizerNone).
    #[strum(serialize = "attr.rerun.visualizer")]
    Visualizer,

    /// The archetype is drawn by no visualizer, and that is deliberate rather than forgotten.
    #[strum(serialize = "attr.rerun.visualizer_none")]
    VisualizerNone,
}

/// What the Rust backend does with a type.
#[derive(AsRefStr, Clone, Copy, Debug, Display, EnumIter, PartialEq, Eq, PartialOrd, Ord)]
pub enum RustAttr {
    /// Traits to `#[derive]` on the generated type, on top of the ones every type gets.
    #[strum(serialize = "attr.rust.derive")]
    Derive,

    /// Traits to `#[derive]`, replacing the ones every type gets rather than adding to them.
    #[strum(serialize = "attr.rust.derive_only")]
    DeriveOnly,

    /// Generate `new` as `pub(crate)`, so that the hand-written `_ext.rs` can wrap it.
    #[strum(serialize = "attr.rust.new_pub_crate")]
    NewPubCrate,

    /// The crate the type is generated into, when it is not `re_sdk_types`.
    #[strum(serialize = "attr.rust.override_crate")]
    OverrideCrate,

    /// The `#[repr]` of the generated type.
    #[strum(serialize = "attr.rust.repr")]
    Repr,

    /// Generate a tuple struct rather than a named one.
    #[strum(serialize = "attr.rust.tuple_struct")]
    TupleStruct,
}

/// Which variant of an enum is the default.
///
/// It applies to the type itself rather than to any one language, and it is written as ordinary
/// Rust — `#[default]` — so it has no namespace, and is not one of the [`Attribute`]s.
pub const ATTR_DEFAULT: &str = "default";

/// One attribute, whatever its namespace.
#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, PartialOrd, Ord)]
pub enum Attribute {
    #[strum(to_string = "{0}")]
    Arrow(ArrowAttr),

    #[strum(to_string = "{0}")]
    Cpp(CppAttr),

    #[strum(to_string = "{0}")]
    Docs(DocsAttr),

    #[strum(to_string = "{0}")]
    Python(PythonAttr),

    #[strum(to_string = "{0}")]
    Rerun(RerunAttr),

    #[strum(to_string = "{0}")]
    Rust(RustAttr),
}

impl Attribute {
    /// Every attribute a definition may carry.
    pub fn all() -> impl Iterator<Item = Self> {
        itertools::chain!(
            ArrowAttr::iter().map(Self::Arrow),
            CppAttr::iter().map(Self::Cpp),
            DocsAttr::iter().map(Self::Docs),
            PythonAttr::iter().map(Self::Python),
            RerunAttr::iter().map(Self::Rerun),
            RustAttr::iter().map(Self::Rust),
        )
    }

    /// The attribute that is stored under `name`, e.g. `attr.rust.derive`, if there is one.
    pub fn parse(name: &str) -> Option<Self> {
        Self::all().find(|attribute| attribute.as_ref() == name)
    }
}

impl AsRef<str> for Attribute {
    #[inline]
    fn as_ref(&self) -> &str {
        match self {
            Self::Arrow(attr) => attr.as_ref(),
            Self::Cpp(attr) => attr.as_ref(),
            Self::Docs(attr) => attr.as_ref(),
            Self::Python(attr) => attr.as_ref(),
            Self::Rerun(attr) => attr.as_ref(),
            Self::Rust(attr) => attr.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_are_namespaced_by_their_enum() {
        assert_eq!(RustAttr::TupleStruct.as_ref(), "attr.rust.tuple_struct");
        assert_eq!(
            Attribute::parse("attr.rust.tuple_struct"),
            Some(Attribute::Rust(RustAttr::TupleStruct))
        );
    }

    #[test]
    fn an_attribute_is_written_the_same_way_wherever_it_is_named() {
        let attribute = RustAttr::TupleStruct;
        assert_eq!(attribute.to_string(), attribute.as_ref());
        assert_eq!(
            Attribute::Rust(attribute).to_string(),
            Attribute::Rust(attribute).as_ref()
        );
    }

    #[test]
    fn unknown_attributes_do_not_parse() {
        assert_eq!(Attribute::parse("attr.rust.tuple_structs"), None);
        assert_eq!(Attribute::parse("attr.rust"), None);
        assert_eq!(Attribute::parse(ATTR_DEFAULT), None);
    }
}
