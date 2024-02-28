//! Implements the Rust codegen pass.

use proc_macro2::TokenStream;

use crate::{ElementType, Object, ObjectKind, Type, ATTR_RUST_TUPLE_STRUCT};

// ---

pub fn is_tuple_struct_from_obj(obj: &Object) -> bool {
    if !obj.is_struct() {
        return false;
    }

    let is_tuple_struct = obj.kind == ObjectKind::Component
        || (obj.is_struct() && obj.try_get_attr::<String>(ATTR_RUST_TUPLE_STRUCT).is_some());

    assert!(
        !is_tuple_struct || obj.fields.len() == 1,
        "`{ATTR_RUST_TUPLE_STRUCT}` is only supported for objects with a single field, but {} has {}",
        obj.fqname,
        obj.fields.len(),
    );

    is_tuple_struct
}

pub fn iter_archetype_components<'a>(
    obj: &'a Object,
    requirement_attr_value: &'static str,
) -> impl Iterator<Item = String> + 'a {
    assert_eq!(ObjectKind::Archetype, obj.kind);
    obj.fields.iter().filter_map(move |field| {
        field
            .try_get_attr::<String>(requirement_attr_value)
            .map(|_| match &field.typ {
                Type::Object(fqname) => fqname.clone(),
                Type::Vector { elem_type } => match elem_type {
                    ElementType::Object(fqname) => fqname.clone(),
                    _ => {
                        panic!("archetype field must be an object/union or an array/vector of such")
                    }
                },
                _ => panic!("archetype field must be an object/union or an array/vector of such"),
            })
    })
}

pub fn string_from_quoted(acc: &TokenStream) -> String {
    re_tracing::profile_function!();

    // We format using `prettyplease` because there are situations with
    // very long lines that `cargo fmt` fails on.
    // See https://github.com/dtolnay/prettyplease for more info.
    let string = prettyplease::unparse(
        &syn::parse_file(&acc.to_string()).expect("Generated Rust code did not parse"),
    );

    // `prettyplease` formats docstrings weirdly, like so:
    //
    // struct Foo {
    //     ///No leading space
    //     bar: i32,
    //     ///And no empty space before the first line
    //     ///of the doscstring
    //     baz: f64,
    // }
    //
    // We fix that here,
    // while also adding blank lines before functions and `impl` blocks.

    let mut output = String::default();
    let mut is_in_docstring = false;
    let mut prev_line_was_attr = false;

    for line in string.split('\n') {
        if let Some(slashes) = line.find("///") {
            let leading_spaces = &line[..slashes];
            if leading_spaces.trim().is_empty() {
                // This is a docstring

                if !is_in_docstring {
                    output.push('\n');
                }
                let comment = &line[slashes + 3..];
                output.push_str(leading_spaces);
                output.push_str("///");
                if !comment.starts_with(char::is_whitespace) {
                    output.push(' ');
                }
                output.push_str(comment);
                output.push('\n');

                prev_line_was_attr = false;
                is_in_docstring = true;

                continue;
            }
        }

        is_in_docstring = false;

        // Insert some extra newlines before functions and `impl` blocks:
        let trimmed = line.trim_start();

        let line_is_attr = trimmed.starts_with("#[allow(") || trimmed.starts_with("#[inline]");

        if !prev_line_was_attr && line_is_attr {
            output.push('\n');
        }

        if !prev_line_was_attr
            && (trimmed.starts_with("const ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("impl<")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("static ")
                || trimmed.starts_with("::re_types_core::macros"))
        {
            output.push('\n');
        }

        output.push_str(line);
        output.push('\n');
        prev_line_was_attr = line_is_attr;
    }

    output
}
