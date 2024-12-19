use std::collections::{BTreeMap, BTreeSet, HashMap};

use camino::Utf8PathBuf;
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    codegen::{autogen_warning, Target},
    ObjectKind, Objects, Reporter, ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RUST_DERIVE,
    ATTR_RUST_DERIVE_ONLY,
};

use super::util::{append_tokens, doc_as_lines};

/// Generate reflection about components and archetypes.
pub fn generate_reflection(
    reporter: &Reporter,
    objects: &Objects,
    extension_contents_for_fqname: &HashMap<String, String>,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    // Put into its own subfolder since codegen is set up in a way that it thinks that everything
    // inside the folder is either generated or an extension to the generated code.
    // This way we don't have to build an exception just for this file.
    let path = Utf8PathBuf::from("crates/store/re_types/src/reflection/mod.rs");

    let mut imports = BTreeSet::new();
    let component_reflection = generate_component_reflection(
        reporter,
        objects,
        extension_contents_for_fqname,
        &mut imports,
    );
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
            Component,
            Loggable as _,
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

        #[doc = "Generates reflection about all known components."]
        #[doc = ""]
        #[doc = "Call only once and reuse the results."]
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
    extension_contents_for_fqname: &HashMap<String, String>,
    imports: &mut BTreeSet<String>,
) -> TokenStream {
    let mut quoted_pairs = Vec::new();

    for obj in objects
        .objects_of_kind(ObjectKind::Component)
        .filter(|obj| !obj.is_testing())
    {
        let crate_name = patched_crate_name(&obj.crate_name());
        if let Some(scope) = obj.scope() {
            imports.insert(format!("{crate_name}::{scope}::components::*"));
        } else {
            imports.insert(format!("{crate_name}::components::*"));
        }

        let type_name = format_ident!("{}", obj.name);

        let quoted_name = if true {
            quote!( <#type_name as Component>::name() )
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

        // Emit custom placeholder if there's a default implementation
        let auto_derive_default = obj.is_enum() // All enums have default values currently!
            || obj
                .try_get_attr::<String>(ATTR_RUST_DERIVE_ONLY)
                .or_else(|| obj.try_get_attr::<String>(ATTR_RUST_DERIVE))
                .map_or(false, |derives| derives.contains("Default"));
        let has_custom_default_impl =
            extension_contents_for_fqname
                .get(&obj.fqname)
                .map_or(false, |contents| {
                    contents.contains(&format!("impl Default for {}", &obj.name))
                        || contents.contains(&format!("impl Default for super::{}", &obj.name))
                });
        let custom_placeholder = if auto_derive_default || has_custom_default_impl {
            quote! { Some(#type_name::default().to_arrow()?) }
        } else {
            quote! { None }
        };

        let quoted_reflection = quote! {
            ComponentReflection {
                docstring_md: #docstring_md,
                custom_placeholder: #custom_placeholder,
                datatype: #type_name::arrow2_datatype(),
            }
        };
        quoted_pairs.push(quote! { (#quoted_name, #quoted_reflection) });
    }

    quote! {
        #[doc = "Generates reflection about all known components."]
        #[doc = ""]
        #[doc = "Call only once and reuse the results."]
        #[allow(deprecated)]
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
            let name = &field.name;
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
                    name: #name,
                    display_name: #display_name,
                    component_name: #component_name.into(),
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
            // because it is very loong and has embedded examples etc.
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

        let scope = if let Some(scope) = obj.scope() {
            quote!(Some(#scope))
        } else {
            quote!(None)
        };

        let quoted_view_types = obj
            .archetype_view_types()
            .unwrap_or_default()
            .iter()
            .map(|view_type| {
                let view_name = &view_type.view_name;
                quote! { #view_name }
            })
            .collect_vec();

        let quoted_archetype_reflection = quote! {
            ArchetypeReflection {
                display_name: #display_name,

                scope: #scope,

                view_types: &[
                    #(#quoted_view_types,)*
                ],

                fields: vec![
                    #(#quoted_field_reflections,)*
                ],
            }
        };
        quoted_pairs.push(quote! { (#quoted_name, #quoted_archetype_reflection) });
    }

    quote! {
        #[doc = "Generates reflection about all known archetypes."]
        #[doc = ""]
        #[doc = "Call only once and reuse the results."]
        fn generate_archetype_reflection() -> ArchetypeReflectionMap {
            re_tracing::profile_function!();
            let array = [
                #(#quoted_pairs,)*
            ];
            ArchetypeReflectionMap::from_iter(array)
        }
    }
}

/// Returns `crate_name` as is, unless it's `re_types`, in which case it's replace by `crate`,
/// because that's where this code lives.
fn patched_crate_name(crate_name: &str) -> String {
    if crate_name == "re_types" {
        "crate".to_owned()
    } else {
        crate_name.to_owned()
    }
}
