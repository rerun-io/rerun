//! The Rust frontend: turns a tree of Rust type definitions into the IDL-agnostic [`Objects`] IR.
//!
//! # The subset
//!
//! Definitions are a deliberately small subset of Rust. Everything outside it is rejected with an
//! error at `path:line:column`.
//!
//! Allowed: `struct`, tuple `struct`, and `enum` items, carrying doc comments, `#[repr(…)]`, and
//! Rerun's own `#[rerun(…)]` / `#[rust(…)]` / `#[python(…)]` / `#[cpp(…)]` / `#[docs(…)]` /
//! `#[arrow(…)]` annotations. Rejected: generics, lifetimes, references, `impl`, `fn`, `trait`,
//! `const`, `static`, `use`, and anything else.
//!
//! # Names
//!
//! A definition's package comes from its path:
//! `re_type_definitions/rerun/components/position3d.def.rs`
//! declares types in `rerun.components`. Definitions refer to each other by fully-qualified name
//! with `::` for `.` — `rerun::components::Position3D` — and never contain a `use` statement.
//! See `re_types_builder_prelude` for how that resolves for rustc.
//!
//! # Annotations
//!
//! The mapping to the [`Attributes`] used by the rest of the pipeline is purely mechanical, with no
//! rename table:
//!
//! | Written               | Attribute                       |
//! | --------------------- | ------------------------------- |
//! | `#[ns(key)]`          | `attr.ns.key`, no value         |
//! | `#[ns(key = "value")]`| `attr.ns.key` = `value`         |
//! | `#[ns(key(a, b::c))]` | `attr.ns.key` = `a, b::c`       |
//! | `#[default]`          | `default`, no value             |
//!
//! Nullability and field order are ordinary Rust, and have no annotation of their own: a nullable
//! field is an `Option<T>`, and field order is source order.
//!
//! Every definition file must also open with [`BANNER`], for whoever — or whatever — reads one
//! without noticing the `.def.rs` in its name.

use std::collections::{BTreeMap, btree_map::Entry};

use camino::{Utf8Path, Utf8PathBuf};
use syn::spanned::Spanned;

use crate::{
    Attribute, Docs, ElementType, Object, ObjectClass, ObjectField, ObjectKind, Objects, Reporter,
    RerunAttr, Type,
};

use super::{Attributes, EnumIntegerType, State};

/// Something was wrong, and it has already been reported to the [`Reporter`].
///
/// The frontend reports every problem it finds instead of stopping at the first one: a bad field is
/// dropped from its struct, a bad struct is dropped from its file, and everything else is parsed as
/// usual. The [`Reporter`] collects the errors, and codegen fails at the very end, so one run tells
/// you about all the mistakes rather than the first one.
///
/// This is why the type carries no payload: by the time it reaches a caller, the user has already
/// been told what went wrong and where. It exists to keep "there is nothing here" ([`Option::None`],
/// e.g. a missing `#[repr]`) apart from "this is broken" in the signatures.
struct Fail;

/// [`Fail`] is the only error in this module, so it is also the default one.
type Result<T, E = Fail> = std::result::Result<T, E>;

// --- Entry point ---

impl Objects {
    /// Runs the semantic pass on a tree of Rust type definitions.
    ///
    /// `definitions_dir` is the root of the definition tree; a type's package name is its path
    /// relative to that root, so `rerun/components/position3d.def.rs` declares `rerun.components`.
    pub fn from_rust_definitions(
        reporter: &Reporter,
        definitions_dir: impl AsRef<Utf8Path>,
    ) -> Self {
        let definitions_dir = definitions_dir.as_ref();

        let mut this = Self::default();

        for filepath in definition_files(reporter, definitions_dir) {
            let Some(pkg_name) = package_name_of(definitions_dir, &filepath) else {
                reporter.error_file(&filepath, "Definition is not inside a package directory");
                continue;
            };

            let contents = match std::fs::read_to_string(&filepath) {
                Ok(contents) => contents,
                Err(err) => {
                    reporter.error_file(&filepath, err);
                    continue;
                }
            };

            check_banner(reporter, &filepath, &contents);

            for object in parse_file(reporter, &filepath, &pkg_name, &contents) {
                this.objects.insert(object.fqname.clone(), object);
            }
        }

        this.validate(reporter);

        this
    }
}

/// What a definition file's name ends with, e.g. `position3d.def.rs`.
///
/// Definitions are ordinary Rust as far as rustc is concerned, so the name is the only thing that
/// tells them apart from the module tree they sit in, and from every other `.rs` file in the repo.
pub(crate) const DEFINITION_SUFFIX: &str = ".def.rs";

/// `…/components/position3d.def.rs` -> `position3d`, and `None` for anything that is not a
/// definition file.
///
/// The module name is the file name without [`DEFINITION_SUFFIX`], so that the definitions are
/// declared as `#[path = "position3d.def.rs"] mod position3d;`.
pub(crate) fn definition_module_name(filepath: &Utf8Path) -> Option<&str> {
    let name = filepath.file_name()?.strip_suffix(DEFINITION_SUFFIX)?;
    (!name.is_empty()).then_some(name)
}

/// Every definition file under `definitions_dir`, sorted.
///
/// Only [`DEFINITION_SUFFIX`] files are definitions; the module tree around them (`lib.rs`, plus a
/// `foo.rs` next to every `foo/`) exists so that rustc and rust-analyzer can see them, and declares
/// no types of its own.
fn definition_files(reporter: &Reporter, definitions_dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut files = Vec::new();
    collect_definition_files(reporter, definitions_dir, &mut files);
    files.sort();
    files
}

fn collect_definition_files(reporter: &Reporter, dir: &Utf8Path, files: &mut Vec<Utf8PathBuf>) {
    let entries = match dir.read_dir_utf8() {
        Ok(entries) => entries,
        Err(err) => {
            reporter.error_file(dir, err);
            return;
        }
    };

    let mut subdirs = Vec::new();
    let mut module_files = Vec::new();

    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.into_path(),
            Err(err) => {
                reporter.error_file(dir, err);
                return;
            }
        };

        if path.is_dir() {
            subdirs.push(path);
        } else if definition_module_name(&path).is_some() {
            files.push(path);
        } else if path.extension() == Some("rs") {
            module_files.push(path);
        }
    }

    // Whatever is left with a `.rs` name is the generated module tree, so make sure that is what it
    // actually is.
    for path in module_files {
        let stem = path.file_stem().unwrap_or_default();
        if stem == "lib" || subdirs.iter().any(|dir| dir.file_name() == Some(stem)) {
            match std::fs::read_to_string(&path) {
                Ok(contents) => check_module_tree_file(reporter, &path, &contents),
                Err(err) => reporter.error_file(&path, err),
            }
        }
    }

    for subdir in subdirs {
        collect_definition_files(reporter, &subdir, files);
    }
}

/// Warns if a file whose name belongs to the generated module tree was written by hand.
///
/// Those names are the generator's — see `codegen::definitions` — so we skip them, and a definition
/// put there would be quietly ignored rather than reaching any SDK.
fn check_module_tree_file(reporter: &Reporter, filepath: &Utf8Path, contents: &str) {
    // Every generated file opens with the `autogen_warning!`.
    if contents.starts_with("// DO NOT EDIT!") {
        return;
    }

    reporter.warn_no_context(format!(
        "{filepath}:1:1: This file name belongs to the generated module tree, so anything defined \
         here is ignored. Give the file a name of its own, or move it into the directory of the \
         same name."
    ));
}

/// `…/re_type_definitions` + `…/re_type_definitions/rerun/components/position3d.def.rs`
/// -> `rerun.components`.
fn package_name_of(definitions_dir: &Utf8Path, filepath: &Utf8Path) -> Option<String> {
    let relative = filepath.strip_prefix(definitions_dir).ok()?;
    let dir = relative.parent()?;
    if dir.as_str().is_empty() {
        return None;
    }
    Some(dir.as_str().replace('/', "."))
}

// --- Files ---

/// What every definition file must open with.
///
/// The `.def.rs` name already says this is a definition, but a reader that only ever sees the
/// contents — a search hit, a code review, a language model — has nothing else to go on.
///
/// Plain `//`, not `//!`, so that neither rustdoc nor [`Docs`] mistakes it for a docstring.
pub(crate) const BANNER: &[&str] = &[
    "// This is a Rerun type definition for the SDK, not executable code.",
    "// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.",
];

fn check_banner(reporter: &Reporter, filepath: &Utf8Path, contents: &str) {
    if contents.lines().take(BANNER.len()).collect::<Vec<_>>() == BANNER {
        return;
    }

    reporter.warn_no_context(format!(
        "{filepath}:1:1: A type definition must open with this banner:\n{}",
        BANNER.join("\n")
    ));
}

pub(crate) fn parse_file(
    reporter: &Reporter,
    filepath: &Utf8Path,
    pkg_name: &str,
    contents: &str,
) -> Vec<Object> {
    let file = match syn::parse_file(contents) {
        Ok(file) => file,
        Err(err) => {
            reporter.error_file(filepath, err);
            return Vec::new();
        }
    };

    let parser = Parser {
        reporter,
        filepath: filepath.to_owned(),
        virtpath: virtpath_of(pkg_name, filepath),
        pkg_name: pkg_name.to_owned(),
    };

    file.items
        .iter()
        .filter_map(|item| parser.parse_item(item).ok())
        .collect()
}

/// The path we show in diagnostics from the rest of the pipeline, e.g.
/// `//rerun/components/position3d.def.rs`.
///
/// It is the package path with a `//` root, which is what makes it stable no matter where the
/// definitions crate sits in the repository.
fn virtpath_of(pkg_name: &str, filepath: &Utf8Path) -> String {
    let file_name = filepath.file_name().unwrap_or_default();
    format!("//{}/{file_name}", pkg_name.replace('.', "/"))
}

struct Parser<'a> {
    reporter: &'a Reporter,
    filepath: Utf8PathBuf,
    virtpath: String,
    pkg_name: String,
}

impl Parser<'_> {
    /// Reports an error at `span`, as `path:line:column`, so that it is clickable in a terminal.
    fn error(&self, span: proc_macro2::Span, text: impl std::fmt::Display) {
        let start = span.start();
        self.reporter.error_any(format!(
            "{}:{}:{}: {text}",
            self.filepath,
            start.line,
            start.column + 1
        ));
    }

    // --- Items ---

    fn parse_item(&self, item: &syn::Item) -> Result<Object> {
        match item {
            syn::Item::Struct(item) => self.parse_struct(item),
            syn::Item::Enum(item) => Ok(self.parse_enum(item)),

            other => {
                self.error(
                    Spanned::span(other),
                    "Only `struct` and `enum` definitions are allowed here",
                );
                Err(Fail)
            }
        }
    }

    fn parse_struct(&self, item: &syn::ItemStruct) -> Result<Object> {
        self.reject_generics(&item.generics);

        let fqname = format!("{}.{}", self.pkg_name, item.ident);
        let attrs = self.parse_attributes(&item.attrs, &fqname);

        let fields = match &item.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .filter_map(|field| self.parse_struct_field(&fqname, field).ok())
                .collect(),

            // A tuple struct's single field is unnamed in Rust, but Arrow needs a name for it,
            // so it gets the conventional one. `#[rust(tuple_struct)]` is what tells the Rust
            // backend to emit a tuple struct rather than a named field.
            syn::Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    self.error(
                        Spanned::span(fields),
                        "Tuple structs must have exactly one field",
                    );
                    return Err(Fail);
                }
                fields
                    .unnamed
                    .iter()
                    .filter_map(|field| self.parse_struct_field(&fqname, field).ok())
                    .collect()
            }

            syn::Fields::Unit => {
                self.error(
                    Spanned::span(item),
                    "Unit structs are not allowed; a type needs at least one field",
                );
                return Err(Fail);
            }
        };

        Ok(self.make_object(
            item.ident.to_string(),
            fqname,
            self.parse_docs(&item.attrs, &item.ident.to_string()),
            attrs,
            fields,
            ObjectClass::Struct,
            Spanned::span(item),
        ))
    }

    fn parse_enum(&self, item: &syn::ItemEnum) -> Object {
        self.reject_generics(&item.generics);

        let fqname = format!("{}.{}", self.pkg_name, item.ident);
        let attrs = self.parse_attributes(&item.attrs, &fqname);

        // The `#[repr(…)]` is what says how the variants are encoded, and so which kind of type
        // this is. It is real Rust, so rustc validates it for us.
        let class = self.parse_repr(&item.attrs, syn::spanned::Spanned::span(item));

        let fields = item
            .variants
            .iter()
            .filter_map(|variant| self.parse_enum_variant(&fqname, variant, class).ok())
            .collect();

        self.make_object(
            item.ident.to_string(),
            fqname,
            self.parse_docs(&item.attrs, &item.ident.to_string()),
            attrs,
            fields,
            class,
            Spanned::span(item),
        )
    }

    #[expect(clippy::too_many_arguments)]
    fn make_object(
        &self,
        name: String,
        fqname: String,
        docs: Docs,
        attrs: Attributes,
        fields: Vec<ObjectField>,
        class: ObjectClass,
        span: proc_macro2::Span,
    ) -> Object {
        let kind = ObjectKind::from_pkg_name(&self.pkg_name, &attrs);
        let state = self.parse_state(&attrs, &fqname, kind, span);

        Object {
            virtpath: self.virtpath.clone(),
            filepath: self.filepath.clone(),
            fqname,
            pkg_name: self.pkg_name.clone(),
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

    /// How far along the type is: experimental, stable or deprecated.
    ///
    /// It reaches the SDKs as doc-comment banners and `#[deprecated]` attributes. Written as
    /// `#[rerun(state = "…")]`; without it, the default depends on the kind and the scope.
    fn parse_state(
        &self,
        attrs: &Attributes,
        fqname: &str,
        kind: ObjectKind,
        span: proc_macro2::Span,
    ) -> State {
        if attrs.has(RerunAttr::State) {
            return State::from_attrs(attrs).unwrap_or_else(|err| {
                self.error(span, err);
                State::Stable
            });
        }

        let scope = attrs
            .get_string(crate::RerunAttr::Scope)
            .or_else(|| (kind == ObjectKind::View).then(|| "blueprint".to_owned()));

        if super::is_testing_fqname(fqname) {
            State::Stable
        } else if scope.as_deref() == Some("blueprint") {
            // All blueprint APIs are considered unstable unless otherwise specified.
            State::Unstable
        } else {
            match kind {
                // TODO(#9427): make the `attr.rerun.state` attribute mandatory
                ObjectKind::Datatype | ObjectKind::Component => State::Stable,
                ObjectKind::Archetype => {
                    self.error(span, format!("Missing attribute `{}`", RerunAttr::State));
                    State::Stable
                }
                ObjectKind::View => State::Unstable,
            }
        }
    }

    // --- Fields ---

    /// One member of a struct, named or not.
    fn parse_struct_field(&self, parent: &str, field: &syn::Field) -> Result<ObjectField> {
        let name = match &field.ident {
            Some(ident) => ident.to_string(),
            // A tuple struct's field is unnamed in Rust, but Arrow needs a name for it, and the
            // backends look this one up by name. It is also what the SDKs call the accessor.
            None => "value".to_owned(),
        };
        let fqname = format!("{parent}#{name}");

        let attrs = self.parse_attributes(&field.attrs, &fqname);
        let MaybeNullable { typ, nullable } = self.parse_field_type(&field.ty)?;

        Ok(ObjectField {
            virtpath: self.virtpath.clone(),
            filepath: self.filepath.clone(),
            fqname,
            pkg_name: parent.to_owned(),
            name: name.clone(),
            enum_or_union_variant_value: None,
            docs: self.parse_docs(&field.attrs, &name),
            state: State::Stable,
            typ,
            attrs,
            is_nullable: nullable,
            datatype: None,
        })
    }

    /// One variant of an `enum`, whether it encodes a C-style enum or a union.
    fn parse_enum_variant(
        &self,
        parent: &str,
        variant: &syn::Variant,
        class: ObjectClass,
    ) -> Result<ObjectField> {
        let name = variant.ident.to_string();
        let fqname = format!("{parent}#{name}");
        let attrs = self.parse_attributes(&variant.attrs, &fqname);

        let payload = match &variant.fields {
            syn::Fields::Unit => MaybeNullable {
                typ: Type::Unit,
                nullable: class.is_enum(),
            },

            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field = &fields.unnamed[0];
                let payload = self.parse_field_type(&field.ty)?;
                if payload.nullable {
                    self.error(
                        Spanned::span(field),
                        "Union variants cannot be nullable. A union already encodes null \
                         through its reserved type-id 0; use a dedicated unit variant if you \
                         want an explicit one",
                    );
                }
                MaybeNullable {
                    nullable: false,
                    ..payload
                }
            }

            other => {
                self.error(
                    Spanned::span(other),
                    "Variants must either have no payload or exactly one unnamed field",
                );
                return Err(Fail);
            }
        };

        // Arrow's union type-ids and our enums' integer values are both wire format, so they are
        // written out explicitly rather than derived from position. `0` is reserved: for unions it
        // is the implicit `_null_markers` variant, for enums it is the invalid value.
        let Some((_, discriminant)) = &variant.discriminant else {
            self.error(
                Spanned::span(variant),
                "Variants need an explicit value, e.g. `= 1`, because it is part of the wire format",
            );
            return Err(Fail);
        };
        let variant_value = self.parse_discriminant(discriminant)?;

        if variant_value == 0 {
            self.error(
                Spanned::span(variant),
                "0 is reserved and cannot be used as a variant value",
            );
        }

        Ok(ObjectField {
            virtpath: self.virtpath.clone(),
            filepath: self.filepath.clone(),
            fqname,
            pkg_name: parent.to_owned(),
            name: name.clone(),
            enum_or_union_variant_value: Some(variant_value),
            docs: self.parse_docs(&variant.attrs, &name),
            state: State::Stable,
            typ: payload.typ,
            attrs,
            is_nullable: payload.nullable,
            datatype: None,
        })
    }

    /// The `= 1` of `Variant = 1`, which is wire format and so must be a plain literal.
    fn parse_discriminant(&self, expr: &syn::Expr) -> Result<u64> {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) = expr
            && let Ok(value) = lit.base10_parse::<u64>()
        {
            return Ok(value);
        }

        self.error(
            Spanned::span(expr),
            "Variant values must be plain integer literals",
        );
        Err(Fail)
    }

    // --- Types ---

    fn parse_field_type(&self, ty: &syn::Type) -> Result<MaybeNullable> {
        if let Some(inner) = as_generic(ty, "Option") {
            if as_generic(inner, "Option").is_some() {
                self.error(
                    Spanned::span(ty),
                    "Nested `Option` is not supported yet — see https://github.com/rerun-io/rerun/issues/2993",
                );
                return Err(Fail);
            }
            return Ok(MaybeNullable {
                typ: self.parse_type(inner)?,
                nullable: true,
            });
        }

        Ok(MaybeNullable {
            typ: self.parse_type(ty)?,
            nullable: false,
        })
    }

    fn parse_type(&self, ty: &syn::Type) -> Result<Type> {
        match ty {
            syn::Type::Path(path) => {
                if let Some(inner) = as_generic(ty, "Vec") {
                    return Ok(Type::Vector {
                        elem_type: self.parse_element_type(inner)?,
                    });
                }
                if as_generic(ty, "Option").is_some() {
                    self.error(
                        Spanned::span(ty),
                        "`Option` is only allowed at the outermost level of a field's type",
                    );
                    return Err(Fail);
                }
                self.parse_named_type(path)
            }

            syn::Type::Array(array) => Ok(Type::Array {
                elem_type: self.parse_element_type(&array.elem)?,
                length: self.parse_array_length(&array.len)?,
            }),

            syn::Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(Type::Unit),

            other => {
                self.error(
                    Spanned::span(other),
                    "Unsupported type; expected a primitive, `String`, `Vec<T>`, `[T; N]`, or a \
                     `rerun::`-rooted path",
                );
                Err(Fail)
            }
        }
    }

    fn parse_element_type(&self, ty: &syn::Type) -> Result<ElementType> {
        let typ = self.parse_type(ty)?;
        typ.to_element_type().ok_or_else(|| {
            self.error(
                Spanned::span(ty),
                format!("{typ:?} cannot be used as an array or vector element"),
            );
            Fail
        })
    }

    fn parse_array_length(&self, expr: &syn::Expr) -> Result<usize> {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) = expr
            && let Ok(length) = lit.base10_parse::<usize>()
        {
            return Ok(length);
        }

        self.error(
            Spanned::span(expr),
            "Array lengths must be plain integer literals",
        );
        Err(Fail)
    }

    /// A primitive, `String`, or a `rerun::`-rooted path to another definition.
    fn parse_named_type(&self, path: &syn::TypePath) -> Result<Type> {
        if path.qself.is_some() {
            self.error(Spanned::span(path), "Qualified paths are not allowed");
            return Err(Fail);
        }

        let segments: Vec<String> = path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();

        if path
            .path
            .segments
            .iter()
            .any(|segment| !segment.arguments.is_none())
        {
            self.error(
                Spanned::span(path),
                "Generic parameters are only allowed on `Vec` and `Option`",
            );
            return Err(Fail);
        }

        if segments.len() == 1 {
            return match segments[0].as_str() {
                "bool" => Ok(Type::Bool),
                "u8" => Ok(Type::UInt8),
                "u16" => Ok(Type::UInt16),
                "u32" => Ok(Type::UInt32),
                "u64" => Ok(Type::UInt64),
                "i8" => Ok(Type::Int8),
                "i16" => Ok(Type::Int16),
                "i32" => Ok(Type::Int32),
                "i64" => Ok(Type::Int64),
                "f32" => Ok(Type::Float32),
                "f64" => Ok(Type::Float64),
                "String" => Ok(Type::String),

                other => {
                    self.error(
                        Spanned::span(path),
                        format!(
                            "Unknown type `{other}`. Refer to other definitions by their full \
                             path, e.g. `rerun::datatypes::Vec3D`"
                        ),
                    );
                    Err(Fail)
                }
            };
        }

        if segments[0] != "rerun" {
            self.error(
                Spanned::span(path),
                format!(
                    "Paths must be rooted at `rerun::`, got `{}`",
                    segments.join("::")
                ),
            );
            return Err(Fail);
        }

        // The two types with no spelling in plain Rust, re-exported at the definitions crate root
        // so that the `rerun::`-rooted rule holds without exceptions.
        // See `re_types_builder_prelude`.
        if segments.len() == 2 {
            match segments[1].as_str() {
                "f16" => return Ok(Type::Float16),
                "Binary" => return Ok(Type::Binary),
                _ => {}
            }
        }

        // A fully-qualified name is just the path with `::` swapped for `.`.
        Ok(Type::Object {
            fqname: segments.join("."),
        })
    }

    // --- Attributes and docs ---

    /// See the [module docs](self) for the mapping.
    fn parse_attributes(&self, attrs: &[syn::Attribute], owner: &str) -> Attributes {
        let mut parsed = BTreeMap::new();

        for attr in attrs {
            let Some(namespace) = attribute_name(attr) else {
                self.error(Spanned::span(attr), "Unexpected attribute path");
                continue;
            };

            match namespace.as_str() {
                // - `doc` is handled by `parse_docs`.
                // - `repr` is real Rust, kept so that rustc validates it. On an enum it also
                //   says how the variants are encoded; see `parse_repr`.
                // - `rerun_type` is the attribute macro that lets rustc accept everything else.
                //   It says nothing about the type. See `re_types_builder_macros`.
                "doc" | "repr" | "rerun_type" => {}

                // Which variant of an enum is the default. It applies to the type itself rather
                // than to any one language, so it has no namespace.
                crate::ATTR_DEFAULT => {
                    if !matches!(attr.meta, syn::Meta::Path(_)) {
                        self.error(Spanned::span(attr), "`#[default]` takes no arguments");
                    }
                    self.insert_attribute(
                        &mut parsed,
                        crate::ATTR_DEFAULT.to_owned(),
                        None,
                        Spanned::span(attr),
                    );
                }

                "arrow" | "cpp" | "docs" | "python" | "rerun" | "rust" => {
                    self.parse_namespaced_attribute(attr, &namespace, owner, &mut parsed);
                }

                other => {
                    self.error(Spanned::span(attr), format!("Unknown attribute `{other}`"));
                }
            }
        }

        Attributes(parsed)
    }

    /// Records one attribute, rejecting a repeat of one we already have.
    ///
    /// Without this, the last spelling would silently win, and the reader of the definition would
    /// have no way of telling which one that is.
    fn insert_attribute(
        &self,
        parsed: &mut BTreeMap<String, Option<String>>,
        name: String,
        value: Option<String>,
        span: proc_macro2::Span,
    ) {
        match parsed.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
            Entry::Occupied(entry) => self.error(
                span,
                format!("`{}` is set more than once", entry.key().as_str()),
            ),
        }
    }

    fn parse_namespaced_attribute(
        &self,
        attr: &syn::Attribute,
        namespace: &str,
        owner: &str,
        parsed: &mut BTreeMap<String, Option<String>>,
    ) {
        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        );

        let nested = match nested {
            Ok(nested) => nested,
            Err(err) => {
                self.error(Spanned::span(attr), err);
                return;
            }
        };

        for meta in nested {
            let Some(key) = meta.path().get_ident().map(|ident| ident.to_string()) else {
                self.error(Spanned::span(&meta), "Expected a plain name");
                continue;
            };
            let name = format!("attr.{namespace}.{key}");

            let span = Spanned::span(&meta);

            if Attribute::parse(&name).is_none() {
                self.error(
                    span,
                    format!("Unknown attribute `{key}` in `#[{namespace}(…)]`."),
                );
                continue;
            }

            match &meta {
                syn::Meta::Path(_) => {
                    self.insert_attribute(parsed, name, None, span);
                }

                syn::Meta::NameValue(name_value) => match &name_value.value {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit),
                        ..
                    }) => {
                        self.insert_attribute(parsed, name, Some(lit.value()), span);
                    }

                    other => self.error(
                        Spanned::span(other),
                        format!("`{name}` on `{owner}` must be given a string literal"),
                    ),
                },

                // A list of paths, e.g. `derive(Default, Copy, bytemuck::Pod)`. The backends want
                // it as the comma-separated string they would emit anyway, so flatten it here.
                syn::Meta::List(list) => {
                    if let Ok(paths) = self.parse_path_list(list) {
                        self.insert_attribute(parsed, name, Some(paths.join(", ")), span);
                    }
                }
            }
        }
    }

    /// The paths inside e.g. `derive(Default, Copy, bytemuck::Pod)`, rendered verbatim.
    fn parse_path_list(&self, list: &syn::MetaList) -> Result<Vec<String>> {
        let nested = list.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        );

        let nested = match nested {
            Ok(nested) => nested,
            Err(err) => {
                self.error(Spanned::span(list), err);
                return Err(Fail);
            }
        };

        let mut paths = Vec::with_capacity(nested.len());
        for meta in &nested {
            let syn::Meta::Path(path) = meta else {
                self.error(
                    Spanned::span(meta),
                    "Expected a path, e.g. `Copy` or `bytemuck::Pod`",
                );
                return Err(Fail);
            };
            // Verbatim, leading `::` and all: a path is emitted into generated code as written.
            let leading = if path.leading_colon.is_some() {
                "::"
            } else {
                ""
            };
            paths.push(format!(
                "{leading}{}",
                path.segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
            ));
        }

        Ok(paths)
    }

    /// How an `enum`'s variants are encoded, from its `#[repr(…)]`.
    ///
    /// `#[repr]` is real Rust, so rustc checks that every variant value fits — and it is also what
    /// makes an explicit value legal on a variant that carries a payload.
    fn parse_repr(&self, attrs: &[syn::Attribute], span: proc_macro2::Span) -> ObjectClass {
        for attr in attrs {
            if !attr.path().is_ident("repr") {
                continue;
            }

            let nested = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            );
            let nested = match nested {
                Ok(nested) => nested,
                Err(err) => {
                    self.error(Spanned::span(attr), err);
                    continue;
                }
            };

            for meta in nested {
                let Some(ident) = meta.path().get_ident() else {
                    self.error(Spanned::span(&meta), "Expected a plain name, e.g. `u8`");
                    continue;
                };
                match ident.to_string().as_str() {
                    // A C-style enum, encoded as a primitive integer array.
                    "u8" => return ObjectClass::Enum(EnumIntegerType::U8),
                    "u16" => return ObjectClass::Enum(EnumIntegerType::U16),
                    "u32" => return ObjectClass::Enum(EnumIntegerType::U32),
                    "u64" => return ObjectClass::Enum(EnumIntegerType::U64),

                    // A sum type, encoded as a dense Arrow union, whose type-ids are `i8`.
                    "i8" => return ObjectClass::Union,

                    "i16" | "i32" | "i64" | "isize" | "usize" => self.error(
                        Spanned::span(&meta),
                        "An enum is either `#[repr(i8)]`, making it a sum type encoded as an \
                         Arrow union, or `#[repr(uN)]`, making it a C-style enum encoded as that \
                         integer",
                    ),

                    _ => {}
                }
            }
        }

        self.error(
            span,
            "Enums need a `#[repr(…)]`: `#[repr(i8)]` for a sum type encoded as an Arrow union, \
             or `#[repr(uN)]` for a C-style enum encoded as that integer",
        );

        // Keep parsing; a union places the fewest demands on the variants.
        ObjectClass::Union
    }

    fn parse_docs(&self, attrs: &[syn::Attribute], name: &str) -> Docs {
        let mut lines = Vec::new();

        for attr in attrs {
            if !attr.path().is_ident("doc") {
                continue;
            }
            let syn::Meta::NameValue(name_value) = &attr.meta else {
                self.error(
                    Spanned::span(attr),
                    "Documentation must be written as `///` comments",
                );
                continue;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit),
                ..
            }) = &name_value.value
            else {
                self.error(
                    Spanned::span(&name_value.value),
                    "A doc comment must be a plain string literal",
                );
                continue;
            };
            // `syn` hands us the text after `///`, leading space included, which is exactly what
            // `Docs` expects.
            lines.push(lit.value());
        }

        Docs::from_lines(
            self.reporter,
            &self.virtpath,
            name,
            lines.iter().map(String::as_str),
        )
    }

    // --- Rejections ---

    fn reject_generics(&self, generics: &syn::Generics) {
        if !generics.params.is_empty() {
            self.error(
                Spanned::span(generics),
                "Generic parameters and lifetimes are not supported yet — \
                 see https://github.com/rerun-io/rerun/issues/7049",
            );
        }
        if let Some(clause) = &generics.where_clause {
            self.error(Spanned::span(clause), "`where` clauses are not allowed");
        }
    }
}

/// A field's type, plus whether the field may be left unset.
///
/// Nullability is written as `Option<T>` rather than as an annotation, so the two fall out of
/// parsing together.
struct MaybeNullable {
    typ: Type,
    nullable: bool,
}

/// The name an attribute is addressed by.
///
/// Definitions contain no `use` statements, so `#[rerun_type]` is written `#[rerun::rerun_type]`
/// — the same `rerun::`-rooted form as everything else. Both spellings resolve to `rerun_type`.
fn attribute_name(attr: &syn::Attribute) -> Option<String> {
    let segments: Vec<String> = attr
        .path()
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();

    match segments.as_slice() {
        [name] => Some(name.clone()),
        [root, name] if root == "rerun" => Some(name.clone()),
        _ => None,
    }
}

/// `Vec<T>` -> `Some(T)` for `as_generic(ty, "Vec")`.
///
/// The name must be written bare: `Vec` and `Option` come from the Rust prelude, so
/// `my_very_cool::Vec<T>` is something else entirely, and is left to [`Parser::parse_named_type`]
/// to reject.
fn as_generic<'a>(ty: &'a syn::Type, wrapper: &str) -> Option<&'a syn::Type> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return None;
    }

    if path.path.segments.len() != 1 {
        return None;
    }

    let segment = path.path.segments.first()?;
    if segment.ident != wrapper {
        return None;
    }

    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }

    match arguments.args.first()? {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses `contents` as if it were `definitions/<pkg>/test.def.rs`, returning the objects it
    /// declares and any errors reported along the way.
    fn parse(pkg_name: &str, contents: &str) -> (Vec<Object>, Vec<String>) {
        let (report, reporter) = crate::report::init();
        let filepath = Utf8PathBuf::from(format!(
            "/definitions/{}/test{DEFINITION_SUFFIX}",
            pkg_name.replace('.', "/")
        ));
        let objects = parse_file(&reporter, &filepath, pkg_name, contents);
        let errors = report.drain_errors();
        (objects, errors)
    }

    /// Parses a definition that is expected to be accepted.
    fn parse_ok(pkg_name: &str, contents: &str) -> Vec<Object> {
        let (objects, errors) = parse(pkg_name, contents);
        assert!(errors.is_empty(), "Expected no errors, got: {errors:#?}");
        objects
    }

    /// Parses a definition that is expected to be rejected, returning the first error.
    fn parse_err(pkg_name: &str, contents: &str) -> String {
        let (_, errors) = parse(pkg_name, contents);
        assert!(!errors.is_empty(), "Expected an error, got none");
        errors[0].clone()
    }

    fn field_types(object: &Object) -> Vec<(&str, &Type, bool)> {
        object
            .fields
            .iter()
            .map(|field| (field.name.as_str(), &field.typ, field.is_nullable))
            .collect()
    }

    #[test]
    fn struct_with_named_fields() {
        let objects = parse_ok(
            "rerun.datatypes",
            r#"
            #[rerun_type]
            pub struct AnnotationInfo {
                pub id: u16,
                pub label: Option<String>,
                pub color: Option<rerun::datatypes::Rgba32>,
            }
            "#,
        );

        assert_eq!(objects.len(), 1);
        let object = &objects[0];
        assert_eq!(object.fqname, "rerun.datatypes.AnnotationInfo");
        assert_eq!(object.pkg_name, "rerun.datatypes");
        assert_eq!(object.name, "AnnotationInfo");
        assert_eq!(object.kind, ObjectKind::Datatype);
        assert_eq!(object.class, ObjectClass::Struct);

        assert_eq!(
            field_types(object),
            vec![
                ("id", &Type::UInt16, false),
                ("label", &Type::String, true),
                (
                    "color",
                    &Type::Object {
                        fqname: "rerun.datatypes.Rgba32".to_owned()
                    },
                    true
                ),
            ]
        );

        // Source order is the order; there is no `order` attribute to get wrong.
        assert_eq!(object.fields[0].fqname, "rerun.datatypes.AnnotationInfo#id");
    }

    #[test]
    fn unnamed_tuple_struct_field_is_named_value() {
        let objects = parse_ok(
            "rerun.components",
            r#"
            #[rerun_type]
            pub struct Radius(pub f32);
            "#,
        );

        assert_eq!(
            field_types(&objects[0]),
            vec![("value", &Type::Float32, false)]
        );
    }

    #[test]
    fn every_supported_data_type() {
        let objects = parse_ok(
            "rerun.datatypes",
            r#"
            #[rerun_type]
            pub struct TypeZoo {
                pub boolean: bool,
                pub unsigned: u64,
                pub signed: i8,
                pub half: rerun::f16,
                pub single: f32,
                pub double: f64,
                pub text: String,
                pub bytes: rerun::Binary,
                pub fixed: [f32; 3],
                pub list: Vec<u8>,
                pub nested: [[f32; 4]; 4],
                pub objects: Vec<rerun::datatypes::Vec3D>,
            }
            "#,
        );

        assert_eq!(
            field_types(&objects[0]),
            vec![
                ("boolean", &Type::Bool, false),
                ("unsigned", &Type::UInt64, false),
                ("signed", &Type::Int8, false),
                ("half", &Type::Float16, false),
                ("single", &Type::Float32, false),
                ("double", &Type::Float64, false),
                ("text", &Type::String, false),
                ("bytes", &Type::Binary, false),
                (
                    "fixed",
                    &Type::Array {
                        elem_type: ElementType::Float32,
                        length: 3
                    },
                    false
                ),
                (
                    "list",
                    &Type::Vector {
                        elem_type: ElementType::UInt8
                    },
                    false
                ),
                (
                    "nested",
                    &Type::Array {
                        elem_type: ElementType::Array {
                            elem_type: Box::new(ElementType::Float32),
                            length: 4
                        },
                        length: 4
                    },
                    false
                ),
                (
                    "objects",
                    &Type::Vector {
                        elem_type: ElementType::Object {
                            fqname: "rerun.datatypes.Vec3D".to_owned()
                        }
                    },
                    false
                ),
            ]
        );
    }

    #[test]
    fn c_style_enum() {
        let objects = parse_ok(
            "rerun.components",
            r#"
            #[rerun_type]
            #[repr(u8)]
            pub enum FillMode {
                /// Lines are drawn around the parts of the shape.
                MajorWireframe = 1,
                DenseWireframe = 2,
                Solid = 3,
                #[default]
                TransparentFillMajorWireframe = 4,
            }
            "#,
        );

        let object = &objects[0];
        assert_eq!(object.class, ObjectClass::Enum(EnumIntegerType::U8));
        assert_eq!(object.enum_integer_type(), Some(EnumIntegerType::U8));

        // Values are explicit, and 0 is reserved for the invalid value, so there is no variant
        // occupying it.
        assert_eq!(
            object
                .fields
                .iter()
                .map(|f| (f.name.as_str(), f.enum_or_union_variant_value))
                .collect::<Vec<_>>(),
            vec![
                ("MajorWireframe", Some(1)),
                ("DenseWireframe", Some(2)),
                ("Solid", Some(3)),
                ("TransparentFillMajorWireframe", Some(4)),
            ]
        );

        assert!(object.fields[3].has_attr(crate::ATTR_DEFAULT));
        assert!(!object.fields[0].has_attr(crate::ATTR_DEFAULT));

        // Enum variants carry no payload.
        assert!(
            object
                .fields
                .iter()
                .all(|f| f.typ == Type::Unit && f.is_nullable)
        );
    }

    #[test]
    fn union_with_payloads_and_a_unit_variant() {
        let objects = parse_ok(
            "rerun.datatypes",
            r#"
            #[rerun_type]
            #[repr(i8)]
            pub enum TimeRangeBoundary {
                CursorRelative(rerun::datatypes::TimeInt) = 1,
                Absolute(rerun::datatypes::TimeInt) = 2,

                /// The boundary extends to infinity.
                Infinite = 3,
            }
            "#,
        );

        let object = &objects[0];
        assert_eq!(object.class, ObjectClass::Union);
        assert!(object.is_union());

        assert_eq!(
            field_types(object),
            vec![
                (
                    "CursorRelative",
                    &Type::Object {
                        fqname: "rerun.datatypes.TimeInt".to_owned()
                    },
                    false
                ),
                (
                    "Absolute",
                    &Type::Object {
                        fqname: "rerun.datatypes.TimeInt".to_owned()
                    },
                    false
                ),
                // A unit variant of a union is `Null`, and not nullable — unlike in an enum.
                ("Infinite", &Type::Unit, false),
            ]
        );
    }

    #[test]
    fn attributes_map_mechanically() {
        let objects = parse_ok(
            "rerun.archetypes",
            r#"
            #[rerun_type]
            #[rerun(state = "stable")]
            #[docs(category = "Spatial 3D", view_types = "Spatial3DView")]
            #[rust(derive = "Default, Copy", tuple_struct)]
            #[arrow(transparent)]
            pub struct Points3D {
                #[rerun(required)]
                #[cpp(rename_field = "positions_")]
                pub positions: Vec<rerun::components::Position3D>,
            }
            "#,
        );

        let object = &objects[0];
        assert_eq!(
            object
                .try_get_attr::<String>(crate::RerunAttr::State)
                .as_deref(),
            Some("stable")
        );
        assert_eq!(
            object
                .try_get_attr::<String>(crate::DocsAttr::Category)
                .as_deref(),
            Some("Spatial 3D")
        );
        assert_eq!(
            object
                .try_get_attr::<String>(crate::RustAttr::Derive)
                .as_deref(),
            Some("Default, Copy")
        );
        // A bare name becomes a valueless attribute.
        assert!(object.is_attr_set(crate::RustAttr::TupleStruct));
        assert!(object.is_attr_set(crate::ArrowAttr::Transparent));

        // A path list is emitted verbatim, leading `::` and all.
        let objects = parse_ok(
            "rerun.datatypes",
            r#"
            #[rerun_type]
            #[rust(derive(Copy, bytemuck::Pod, ::serde::Serialize))]
            pub struct Vec3D {
                pub xyz: [f32; 3],
            }
            "#,
        );
        assert_eq!(
            objects[0]
                .try_get_attr::<String>(crate::RustAttr::Derive)
                .as_deref(),
            Some("Copy, bytemuck::Pod, ::serde::Serialize")
        );

        let field = &object.fields[0];
        assert!(field.has_attr(crate::RerunAttr::Required));
        assert_eq!(field.kind(), Some(crate::objects::FieldKind::Required));
        assert_eq!(
            field
                .try_get_attr::<String>(crate::CppAttr::RenameField)
                .as_deref(),
            Some("positions_")
        );
    }

    #[test]
    fn doc_comments_keep_their_tags() {
        let objects = parse_ok(
            "rerun.datatypes",
            r#"
            /// A position in 3D space.
            ///
            /// More detail.
            /// \py Python-only detail.
            #[rerun_type]
            pub struct Vec3D {
                /// The coordinates.
                pub xyz: [f32; 3],
            }
            "#,
        );

        let object = &objects[0];
        assert_eq!(
            object.docs.only_lines_tagged(""),
            vec!["A position in 3D space.", "", "More detail."]
        );
        assert_eq!(
            object.docs.only_lines_tagged("py"),
            vec!["Python-only detail."]
        );
        assert_eq!(
            object.fields[0].docs.only_lines_tagged(""),
            vec!["The coordinates."]
        );
    }

    #[test]
    fn state_defaults_by_kind_and_scope() {
        let datatype = parse_ok("rerun.datatypes", "#[rerun_type] pub struct A(pub f32);");
        assert_eq!(datatype[0].state, State::Stable);

        let blueprint = parse_ok(
            "rerun.blueprint.datatypes",
            r#"#[rerun_type] #[rerun(scope = "blueprint")] pub struct A(pub f32);"#,
        );
        assert_eq!(blueprint[0].state, State::Unstable);

        let deprecated = parse_ok(
            "rerun.datatypes",
            r#"
            #[rerun_type]
            #[rerun(state = "deprecated", deprecated_since = "0.30", deprecated_notice = "Use B")]
            pub struct A(pub f32);
            "#,
        );
        assert_eq!(
            deprecated[0].state,
            State::Deprecated {
                since: "0.30".to_owned(),
                notice: "Use B".to_owned()
            }
        );
    }

    #[test]
    fn archetypes_must_declare_their_state() {
        let error = parse_err(
            "rerun.archetypes",
            "#[rerun_type] pub struct Points3D { pub positions: Vec<rerun::components::Position3D> }",
        );
        assert!(error.contains("attr.rerun.state"), "{error}");
    }

    #[test]
    fn errors_point_at_the_offending_line() {
        let error = parse_err(
            "rerun.datatypes",
            "#[rerun_type]\npub struct A {\n    pub bad: HashMap<u8, u8>,\n}",
        );
        assert!(
            error.starts_with("/definitions/rerun/datatypes/test.def.rs:3:14:"),
            "{error}"
        );
    }

    #[test]
    fn rejects_everything_outside_the_subset() {
        let cases = [
            // (definition, expected substring of the error)
            ("pub fn foo() {}", "Only `struct` and `enum`"),
            ("impl Foo {}", "Only `struct` and `enum`"),
            ("use rerun::datatypes::Vec3D;", "Only `struct` and `enum`"),
            ("pub const N: u8 = 1;", "Only `struct` and `enum`"),
            ("pub struct A<T> { pub a: T }", "Generic parameters"),
            ("pub struct A<'a> { pub a: &'a u8 }", "Generic parameters"),
            ("pub struct A { pub a: &'static str }", "Unsupported type"),
            ("pub struct A;", "Unit structs are not allowed"),
            ("pub struct A(pub u8, pub u8);", "exactly one field"),
            ("pub struct A { pub a: Vec3D }", "Unknown type `Vec3D`"),
            (
                "pub struct A { pub a: std::string::String }",
                "rooted at `rerun::`",
            ),
            (
                "pub struct A { pub a: Option<Option<u8>> }",
                "Nested `Option` is not supported",
            ),
            ("pub struct A { pub a: Vec<Option<u8>> }", "outermost level"),
            ("pub struct A { pub a: [u8; N] }", "plain integer literals"),
            ("#[repr(i8)] pub enum A { B(u8) }", "need an explicit value"),
            ("#[repr(i8)] pub enum A { B(u8) = 0 }", "0 is reserved"),
            ("pub enum A { B = 1 }", "Enums need a `#[repr(…)]`"),
            ("#[repr(i16)] pub enum A { B = 1 }", "either `#[repr(i8)]`"),
            (
                "#[repr(i8)] pub enum A { B { x: u8 } = 1 }",
                "exactly one unnamed field",
            ),
            (
                "#[bogus] pub struct A(pub u8);",
                "Unknown attribute `bogus`",
            ),
            // A known namespace does not make the key inside it known.
            (
                "#[rerun(not_an_attribute)] pub struct A(pub u8);",
                "Unknown attribute `not_an_attribute`",
            ),
            (
                "#[rerun(state = 3)] pub struct A(pub u8);",
                "must be given a string literal",
            ),
            (
                r#"#[rust(derive(Default = "x"))] pub struct A(pub u8);"#,
                "Expected a path",
            ),
            // A repeated attribute, both within one `#[…]` and across two of them.
            (
                r#"#[rust(derive = "Copy", derive = "Clone")] pub struct A(pub u8);"#,
                "`attr.rust.derive` is set more than once",
            ),
            (
                r#"#[rust(derive = "Copy")] #[rust(derive = "Clone")] pub struct A(pub u8);"#,
                "`attr.rust.derive` is set more than once",
            ),
            (
                "#[default] #[default] pub struct A(pub u8);",
                "`default` is set more than once",
            ),
            (
                "#[doc(hidden)] pub struct A(pub u8);",
                "written as `///` comments",
            ),
            // `Vec` and `Option` are the prelude ones, not something with the same last segment.
            (
                "pub struct A { pub a: my_very_cool::Vec<u8> }",
                "Generic parameters are only allowed on `Vec` and `Option`",
            ),
            (
                "pub struct A { pub a: std::option::Option<u8> }",
                "Generic parameters are only allowed on `Vec` and `Option`",
            ),
            (
                r#"#[doc = concat!("a", "b")] pub struct A(pub u8);"#,
                "plain string literal",
            ),
        ];

        for (definition, expected) in cases {
            let error = parse_err("rerun.datatypes", definition);
            assert!(
                error.contains(expected),
                "Expected {expected:?} in error for {definition:?}, got: {error}"
            );
        }
    }

    #[test]
    fn rerun_type_can_be_written_as_a_path() {
        // Definitions contain no `use` statements, so this is the spelling they actually use.
        let objects = parse_ok(
            "rerun.components",
            r#"
            #[rerun::rerun_type]
            #[rerun(state = "stable")]
            pub struct Radius(pub f32);
            "#,
        );
        assert_eq!(objects[0].fqname, "rerun.components.Radius");
        assert!(!objects[0].is_attr_set("attr.rerun.rerun_type"));
    }

    #[test]
    fn only_def_rs_files_are_definitions() {
        let name = |path| definition_module_name(Utf8Path::new(path));

        assert_eq!(
            name("/x/rerun/components/position3d.def.rs"),
            Some("position3d")
        );

        // The module tree, and anything else that happens to sit in the same directory.
        assert_eq!(name("/x/rerun/components.rs"), None);
        assert_eq!(name("/x/rerun/lib.rs"), None);
        assert_eq!(name("/x/rerun/components/position3d.fbs"), None);
        assert_eq!(name("/x/rerun/components/position3d.rs"), None);
        assert_eq!(name("/x/rerun/components/def.rs"), None);
        assert_eq!(name("/x/rerun/components/.def.rs"), None); // No module name left.
    }

    #[test]
    fn definitions_must_open_with_the_banner() {
        fn warning(contents: &str) -> Option<String> {
            let (report, reporter) = crate::report::init();
            let filepath = Utf8Path::new("/definitions/rerun/components/test.def.rs");
            check_banner(&reporter, filepath, contents);
            report.drain_warnings().into_iter().next()
        }

        let banner = BANNER.join("\n");

        assert_eq!(warning(&banner), None);
        assert_eq!(
            warning(&format!(
                "{banner}\n\n#[rerun_type]\npub struct Position3D;\n"
            )),
            None
        );

        let bad = [
            String::new(),                                      // empty
            "#[rerun_type]\npub struct Position3D;".to_owned(), // no banner at all
            BANNER[..BANNER.len() - 1].join("\n"),              // truncated
            banner.replace("not executable code", "not code"),  // reworded
            format!("\n{banner}"),                              // not at the very top
        ];

        for contents in bad {
            assert!(warning(&contents).is_some(), "Should warn: {contents:?}");
        }
    }

    #[test]
    fn package_name_comes_from_the_path() {
        let root = Utf8Path::new("/x/definitions");
        assert_eq!(
            package_name_of(
                root,
                Utf8Path::new("/x/definitions/rerun/components/a.def.rs")
            )
            .as_deref(),
            Some("rerun.components")
        );
        assert_eq!(
            package_name_of(
                root,
                Utf8Path::new("/x/definitions/rerun/blueprint/views/a.def.rs")
            )
            .as_deref(),
            Some("rerun.blueprint.views")
        );
        // A file directly in the root belongs to no package.
        assert_eq!(
            package_name_of(root, Utf8Path::new("/x/definitions/lib.rs")),
            None
        );
    }

    #[test]
    fn hand_written_module_tree_files_warn() {
        fn warning(contents: &str) -> Option<String> {
            let (report, reporter) = crate::report::init();
            let filepath = Utf8Path::new("/definitions/rerun/components.rs");
            check_module_tree_file(&reporter, filepath, contents);
            report.drain_warnings().into_iter().next()
        }

        assert_eq!(
            warning("// DO NOT EDIT! This file was auto-generated by …\n\npub mod components;\n"),
            None
        );
        assert!(warning("#[rerun_type]\npub struct A(pub u8);\n").is_some());
        assert!(warning("").is_some());
    }
}
