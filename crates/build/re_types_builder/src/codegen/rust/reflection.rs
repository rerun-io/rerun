use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    codegen::{autogen_warning, Target},
    ObjectKind, Objects, Reporter, ATTR_RERUN_COMPONENT_REQUIRED,
};

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
    let path = Utf8PathBuf::from("crates/viewer/re_viewer/src/reflection/mod.rs");

    let mut imports = BTreeSet::new();
    let component_reflection = generate_component_reflection(reporter, objects, &mut imports);
    let archetype_reflection = generate_archetype_reflection(reporter, objects);

    let mut code = format!("// {}\n\n", autogen_warning!());
    code.push_str("#![allow(clippy::too_many_lines)]\n");
    code.push_str("#![allow(clippy::wildcard_imports)]\n\n");
    code.push_str("#![allow(unused_imports)]\n");
    for namespace in imports {
        code.push_str(&format!("use {namespace};\n"));
    }

    let quoted_reflection = quote! {
        use re_types_core::{
            ArchetypeName,
            ComponentName,
            Loggable,
            LoggableBatch as _,
            reflection::{
                ArchetypeFieldReflection,
                ArchetypeReflection,
                ArchetypeReflectionMap,
                ComponentReflection,
                ComponentReflectionMap,
                Reflection,
            },
            SerializationError,
        };

        /// Generates reflection about all known components.
        ///
        /// Call only once and reuse the results.
        pub fn generate_reflection() -> Result<Reflection, SerializationError> {
            re_tracing::profile_function!();

            Ok(Reflection {
                components: generate_component_reflection()?,
                archetypes: generate_archetype_reflection(),
            })
        }

        #component_reflection

        #archetype_reflection
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

        let docstring_md = doc_as_lines(
            reporter,
            objects,
            &obj.virtpath,
            &obj.fqname,
            &obj.docs,
            Target::WebDocsMarkdown,
            obj.is_experimental(),
        )
        .join("\n");
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
            re_tracing::profile_function!();
            let array = [
                #(#quoted_pairs,)*
            ];
            Ok(ComponentReflectionMap::from_iter(array))
        }
    }
}

/// Generate reflection about components.
fn generate_archetype_reflection(reporter: &Reporter, objects: &Objects) -> TokenStream {
    let mut quoted_pairs = Vec::new();

    for obj in objects
        .objects_of_kind(ObjectKind::Archetype)
        .filter(|obj| !obj.is_testing())
    {
        let quoted_field_reflections = obj.fields.iter().map(|field| {
            let Some(component_name) = field.typ.fqname() else {
                panic!("archetype field must be an object/union or an array/vector of such")
            };
            let display_name = re_case::to_human_case(&field.name);
            let docstring_md = doc_as_lines(
                reporter,
                objects,
                &field.virtpath,
                &field.fqname,
                &field.docs,
                Target::WebDocsMarkdown,
                obj.is_experimental(),
            )
            .join("\n");
            let required = field.attrs.has(ATTR_RERUN_COMPONENT_REQUIRED);

            quote! {
                ArchetypeFieldReflection {
                    component_name: #component_name.into(),
                    display_name: #display_name,
                    docstring_md: #docstring_md,
                    is_required: #required,
                }
            }
        });

        let fqname = &obj.fqname;
        let quoted_name = quote!( ArchetypeName::new(#fqname) );
        let display_name = re_case::to_human_case(&obj.name);
        if false {
            // We currently skip the docstring for the archetype itself,
            // because it is very loong and has mebedded examples etc.
            // We also never use it.
            doc_as_lines(
                reporter,
                objects,
                &obj.virtpath,
                &obj.fqname,
                &obj.docs,
                Target::WebDocsMarkdown,
                obj.is_experimental(),
            )
            .join("\n");
        }

        let quoted_archetype_reflection = quote! {
            ArchetypeReflection {
                display_name: #display_name,

                fields: vec![
                    #(#quoted_field_reflections,)*
                ],
            }
        };
        quoted_pairs.push(quote! { (#quoted_name, #quoted_archetype_reflection) });
    }

    quote! {
        /// Generates reflection about all known archetypes.
        ///
        /// Call only once and reuse the results.
        fn generate_archetype_reflection() -> ArchetypeReflectionMap {
            re_tracing::profile_function!();
            let array = [
                #(#quoted_pairs,)*
            ];
            ArchetypeReflectionMap::from_iter(array)
        }
    }
}
