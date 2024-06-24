use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{codegen::autogen_warning, ObjectKind, Objects, Reporter};

use super::util::{append_tokens, doc_as_lines};

/// Generate reflection about components and archetypes.
pub fn generate_reflection(
    reporter: &Reporter,
    objects: &Objects,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    // Put into its own subfolder since codegen is set up in a way that it thinks that everything
    // inside the folder is either generated or an extension to the generated code.
    // This way we don't have to build an exception just for this file.
    let path = Utf8PathBuf::from("crates/re_viewer/src/reflection/mod.rs");

    let mut imports = BTreeSet::new();
    let component_reflection = generate_component_reflection(reporter, objects, &mut imports);

    let mut code = format!("// {}\n\n", autogen_warning!());
    code.push_str("#![allow(unused_imports)]\n");
    code.push_str("#![allow(clippy::wildcard_imports)]\n\n");
    for namespace in imports {
        code.push_str(&format!("use {namespace};\n"));
    }

    let quoted_reflection = quote! {
        use re_types_core::{
            external::arrow2,
            ComponentName,
            SerializationError,
            reflection::{Reflection, ComponentReflectionMap, ComponentReflection}
        };

        /// Generates reflection about all known components.
        ///
        /// Call only once and reuse the results.
        pub fn generate_reflection() -> Result<Reflection, SerializationError> {
            use ::re_types_core::{Loggable, LoggableBatch as _};

            re_tracing::profile_function!();

            Ok(Reflection {
                components: generate_component_reflection()?,

                // TODO(emilk): achetypes
            })
        }

        #component_reflection
    };

    let code = append_tokens(reporter, code, &quoted_reflection, &path);

    files_to_write.insert(path, code);
}

/// Generate reflection about components.
fn generate_component_reflection(
    reporter: &Reporter,
    objects: &Objects,
    imports: &mut BTreeSet<String>,
) -> TokenStream {
    let mut quoted_pairs = Vec::new();

    for obj in objects
        .objects_of_kind(ObjectKind::Component)
        .filter(|obj| !obj.is_testing())
    {
        if let Some(scope) = obj.scope() {
            imports.insert(format!("{}::{scope}::components::*", obj.crate_name()));
        } else {
            imports.insert(format!("{}::components::*", obj.crate_name()));
        }

        let type_name = format_ident!("{}", obj.name);

        let quoted_name = if true {
            quote!( <#type_name as Loggable>::name() )
        } else {
            // Works too
            let fqname = &obj.fqname;
            quote!( ComponentName::new(#fqname) )
        };

        let docstring_md = doc_as_lines(reporter, &obj.virtpath, &obj.fqname, &obj.docs).join("\n");
        let quoted_reflection = quote! {
            ComponentReflection {
                docstring_md: #docstring_md,

                placeholder: Some(#type_name::default().to_arrow()?),
            }
        };
        quoted_pairs.push(quote! { (#quoted_name, #quoted_reflection) });
    }

    quote! {
        /// Generates reflection about all known components.
        ///
        /// Call only once and reuse the results.
        fn generate_component_reflection() -> Result<ComponentReflectionMap, SerializationError> {
            use ::re_types_core::{Loggable, LoggableBatch as _};

            re_tracing::profile_function!();
            Ok(ComponentReflectionMap::from_iter([
                #(#quoted_pairs,)*
            ]))
        }
    }
}
