//! Turns the IR back into the Rust definitions that `objects/from_rust.rs` reads.
//!
//! ---
//!
//! **This is a temporary jig and does not need reviewing.** It has already done its job — the 309
//! `.rs` definitions in this PR are its output — and step 5 of the migration deletes it along with
//! the Flatbuffers definitions it reads. Nothing it emits is trusted on faith either: whatever it
//! gets wrong shows up as a diff in `compare_frontends`, which is the thing worth reviewing.
//!
//! It is kept running as part of `pixi run codegen` only so that the two definition trees cannot
//! drift while both exist — an edit to a `.fbs` file regenerates the `.rs` beside it.
//!
//! ---
//!
//! This is the bridge that makes the migration off Flatbuffers a mechanical operation rather than
//! a rewrite: the definitions are transpiled from the *post-semantic-pass* IR, so every wrapper
//! type and `transparent` marker that only ever existed to work around Flatbuffers has already
//! been folded away, and what comes out is what a person would have written by hand.
//!
//! Transpiling is the inverse of parsing, and the two are checked against each other: the same IR
//! must generate byte-identical Rust, Python, C++ and docs whichever frontend produced it.
//!
//! TODO(RR-5384): remove once we've migrated completely from flatbuffers.

use std::fmt::Write as _;

use crate::objects::State;
use crate::{
    ATTR_DEFAULT, ATTR_NULLABLE, ATTR_ORDER, ATTR_RERUN_STATE, ATTR_RUST_DERIVE,
    ATTR_RUST_DERIVE_ONLY, ATTR_TRANSPARENT, Attributes, Docs, ElementType, Object, ObjectClass,
    ObjectField, Reporter, Type,
};

/// Attributes the Rust definitions express as ordinary Rust, and so never write out.
///
/// Nullability is `Option<T>`, order is source order, and `transparent` is what the semantic pass
/// has just finished folding away.
const IMPLICIT: &[&str] = &[ATTR_NULLABLE, ATTR_ORDER, ATTR_TRANSPARENT];

/// What every definition file opens with. See `objects/from_rust.rs`.
const BANNER: &[&str] = crate::objects::from_rust::BANNER;

/// The attributes whose value is a list of paths, written out as one, e.g. `derive(Copy, Default)`.
const PATH_LISTS: &[&str] = &[ATTR_RUST_DERIVE, ATTR_RUST_DERIVE_ONLY];

/// Transpiles all the objects that were declared in a single definition file.
///
/// They are written in the order given, which is the order they were declared in.
pub fn transpile_file(reporter: &Reporter, objects: &[&Object]) -> String {
    let mut out = String::new();

    for line in BANNER {
        writeln!(out, "{line}").ok();
    }

    for object in objects {
        out.push('\n');
        transpile_object(reporter, object, &mut out);
    }

    out
}

fn transpile_object(reporter: &Reporter, object: &Object, out: &mut String) {
    let Object {
        virtpath: _,
        filepath: _,
        fqname,
        pkg_name: _,
        name,
        docs,
        kind: _,
        attrs,
        state,
        fields,
        class,
        datatype: _,
    } = object;

    write_docs(docs, "", out);

    // The attribute macro has to come first: it is what strips all the Rerun attributes below it,
    // which rustc would otherwise refuse to resolve. See `re_types_builder_macros`.
    out.push_str("#[rerun::rerun_type]\n");

    // The repr is what says how the variants are encoded, and so which kind of type this is.
    match class {
        ObjectClass::Struct => {}
        ObjectClass::Enum(integer_type) => {
            writeln!(out, "#[repr({})]", integer_type.type_str()).ok();
        }
        ObjectClass::Union => {
            // Arrow encodes a union's variant as an `i8` type-id.
            out.push_str("#[repr(i8)]\n");
        }
    }

    write_attributes(reporter, fqname, attrs, "", out);

    // The default state depends on the kind and the scope, so writing it out on every object keeps
    // the transpiled definition readable and independent of that.
    if !attrs.has(ATTR_RERUN_STATE) {
        writeln!(out, "#[rerun(state = {:?})]", state_name(state)).ok();
    }

    match class {
        ObjectClass::Struct => {
            writeln!(out, "pub struct {name} {{").ok();
            for field in fields {
                out.push('\n');
                transpile_field(reporter, field, out);
            }
            out.push_str("}\n");
        }

        ObjectClass::Enum(_) | ObjectClass::Union => {
            writeln!(out, "pub enum {name} {{").ok();
            for field in fields {
                out.push('\n');
                transpile_variant(reporter, field, out);
            }
            out.push_str("}\n");
        }
    }
}

fn transpile_field(reporter: &Reporter, field: &ObjectField, out: &mut String) {
    let ObjectField {
        fqname,
        name,
        docs,
        typ,
        attrs,
        is_nullable,
        ..
    } = field;

    write_docs(docs, "    ", out);
    write_attributes(reporter, fqname, attrs, "    ", out);

    let typ = type_name(typ);
    let typ = if *is_nullable {
        format!("Option<{typ}>")
    } else {
        typ
    };

    writeln!(out, "    pub {name}: {typ},").ok();
}

fn transpile_variant(reporter: &Reporter, variant: &ObjectField, out: &mut String) {
    let ObjectField {
        fqname,
        name,
        docs,
        typ,
        attrs,
        enum_or_union_variant_value,
        ..
    } = variant;

    write_docs(docs, "    ", out);
    write_attributes(reporter, fqname, attrs, "    ", out);

    let payload = match typ {
        Type::Unit => String::new(),
        typ => format!("({})", type_name(typ)),
    };

    let Some(value) = enum_or_union_variant_value else {
        reporter.error_any(format!("{fqname}: variant has no value"));
        return;
    };

    // The value is wire format, so it is always written out.
    writeln!(out, "    {name}{payload} = {value},").ok();
}

fn write_docs(docs: &Docs, indent: &str, out: &mut String) {
    for line in docs.to_lines() {
        writeln!(out, "{indent}///{line}").ok();
    }
}

fn write_attributes(
    reporter: &Reporter,
    owner: &str,
    attrs: &Attributes,
    indent: &str,
    out: &mut String,
) {
    for (name, value) in attrs.iter() {
        if IMPLICIT.contains(&name) {
            continue;
        }

        if name == ATTR_DEFAULT {
            writeln!(out, "{indent}#[default]").ok();
            continue;
        }

        let Some((namespace, key)) = name
            .strip_prefix("attr.")
            .and_then(|rest| rest.split_once('.'))
        else {
            reporter.error_any(format!("{owner}: cannot write out the attribute `{name}`"));
            continue;
        };

        match value {
            None => writeln!(out, "{indent}#[{namespace}({key})]").ok(),
            Some(value) if PATH_LISTS.contains(&name) => {
                writeln!(out, "{indent}#[{namespace}({key}({value}))]").ok()
            }
            Some(value) => writeln!(out, "{indent}#[{namespace}({key} = {value:?})]").ok(),
        };
    }
}

fn state_name(state: &State) -> &'static str {
    match state {
        State::Unstable => "unstable",
        State::Stable => "stable",
        // `deprecated_since` and `deprecated_notice` are ordinary attributes, and already written.
        State::Deprecated { .. } => "deprecated",
    }
}

/// A field's type, as Rust.
///
/// Definitions name each other by their fully-qualified name with `::` for `.`, so the two types
/// that have no spelling in plain Rust are named the same way. See `re_types_builder_prelude`.
fn type_name(typ: &Type) -> String {
    match typ {
        Type::Unit => "()".to_owned(),
        Type::UInt8 => "u8".to_owned(),
        Type::UInt16 => "u16".to_owned(),
        Type::UInt32 => "u32".to_owned(),
        Type::UInt64 => "u64".to_owned(),
        Type::Int8 => "i8".to_owned(),
        Type::Int16 => "i16".to_owned(),
        Type::Int32 => "i32".to_owned(),
        Type::Int64 => "i64".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Float16 => "rerun::f16".to_owned(),
        Type::Float32 => "f32".to_owned(),
        Type::Float64 => "f64".to_owned(),
        Type::Binary => "rerun::Binary".to_owned(),
        Type::String => "String".to_owned(),
        Type::Array { elem_type, length } => {
            format!("[{}; {length}]", element_type_name(elem_type))
        }
        Type::Vector { elem_type } => format!("Vec<{}>", element_type_name(elem_type)),
        Type::Object { fqname } => fqname.replace('.', "::"),
    }
}

fn element_type_name(typ: &ElementType) -> String {
    match typ {
        ElementType::Array { elem_type, length } => {
            format!("[{}; {length}]", element_type_name(elem_type))
        }
        typ => type_name(&Type::from(typ.clone())),
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use crate::objects::from_rust::parse_file;

    use super::*;

    const PATH: &str = "/definitions/rerun/datatypes/test.def.rs";
    const PKG: &str = "rerun.datatypes";

    fn parse(contents: &str) -> Vec<Object> {
        let (report, reporter) = crate::report::init();
        let objects = parse_file(&reporter, Utf8Path::new(PATH), PKG, contents);
        let errors = report.drain_errors();
        assert!(errors.is_empty(), "Expected no errors, got: {errors:#?}");
        objects
    }

    /// Parses `contents`, transpiles it back, and returns what came out.
    ///
    /// Also asserts that parsing the result gives the same objects again: whatever a definition
    /// can say, the transpiler has to be able to say it too, or the migration silently drops it.
    fn transpile(contents: &str) -> String {
        let (_report, reporter) = crate::report::init();

        let objects = parse(contents);
        let transpiled = transpile_file(&reporter, &objects.iter().collect::<Vec<_>>());

        assert_eq!(
            format!("{:#?}", parse(&transpiled)),
            format!("{objects:#?}"),
            "Transpiling changed the objects. Transpiled:\n{transpiled}"
        );

        transpiled
    }

    #[test]
    fn struct_with_every_kind_of_field() {
        insta::assert_snapshot!(transpile(r#"
            /// A point in space.
            ///
            /// \py Only in Python.
            #[rerun_type]
            #[rerun(state = "stable")]
            #[rust(derive(Default, Copy, bytemuck::Pod))]
            #[python(aliases = "float")]
            pub struct Everything {
                /// A required component.
                #[rerun(component_required)]
                pub required: rerun::components::Position3D,

                pub optional: Option<f32>,
                pub half: rerun::f16,
                pub bytes: rerun::Binary,
                pub text: String,
                pub fixed: [f64; 3],
                pub list: Vec<u8>,
                pub nested: Vec<[f32; 2]>,
            }
            "#), @r#"
        // This is a Rerun type definition for the SDK, not executable code.
        // It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

        /// A point in space.
        ///
        /// \py Only in Python.
        #[rerun::rerun_type]
        #[python(aliases = "float")]
        #[rerun(state = "stable")]
        #[rust(derive(Default, Copy, bytemuck::Pod))]
        pub struct Everything {

            /// A required component.
            #[rerun(component_required)]
            pub required: rerun::components::Position3D,

            pub optional: Option<f32>,

            pub half: rerun::f16,

            pub bytes: rerun::Binary,

            pub text: String,

            pub fixed: [f64; 3],

            pub list: Vec<u8>,

            pub nested: Vec<[f32; 2]>,
        }
        "#);
    }

    #[test]
    fn c_style_enum_keeps_its_integer_type() {
        insta::assert_snapshot!(transpile(r#"
            #[rerun_type]
            #[repr(u32)]
            #[rerun(state = "stable")]
            pub enum Codec {
                /// H.264
                H264 = 0x61766331,

                #[default]
                H265 = 0x68766331,
            }
            "#), @r#"
        // This is a Rerun type definition for the SDK, not executable code.
        // It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

        #[rerun::rerun_type]
        #[repr(u32)]
        #[rerun(state = "stable")]
        pub enum Codec {

            /// H.264
            H264 = 1635148593,

            #[default]
            H265 = 1752589105,
        }
        "#);
    }

    #[test]
    fn union_keeps_its_type_ids() {
        insta::assert_snapshot!(transpile(r#"
            #[rerun_type]
            #[repr(i8)]
            #[rerun(state = "stable")]
            pub enum TimeRangeBoundary {
                CursorRelative(rerun::datatypes::TimeInt) = 1,

                /// Extends to infinity.
                Infinite = 3,
            }
            "#), @r#"
        // This is a Rerun type definition for the SDK, not executable code.
        // It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

        #[rerun::rerun_type]
        #[repr(i8)]
        #[rerun(state = "stable")]
        pub enum TimeRangeBoundary {

            CursorRelative(rerun::datatypes::TimeInt) = 1,

            /// Extends to infinity.
            Infinite = 3,
        }
        "#);
    }

    #[test]
    fn a_file_can_declare_several_types() {
        let transpiled = transpile(
            r#"
            #[rerun_type]
            #[rerun(state = "stable")]
            pub struct First {
                pub value: u8,
            }

            #[rerun_type]
            #[rerun(state = "stable")]
            pub struct Second {
                pub value: u8,
            }
            "#,
        );

        assert_eq!(transpiled.matches("pub struct").count(), 2);
    }
}
