use std::collections::{BTreeMap, BTreeSet, HashMap};

use camino::Utf8PathBuf;
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::util::{append_tokens, doc_as_lines};
use crate::codegen::{Target, autogen_warning};
use crate::{
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RERUN_COMPONENT_UI_EDITABLE, ATTR_RUST_DERIVE,
    ATTR_RUST_DERIVE_ONLY, ObjectKind, Objects, Reporter,
};

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
    let path = Utf8PathBuf::from("crates/store/re_sdk_types/src/reflection/mod.rs");

    let mut imports = BTreeSet::new();
    let component_reflection = generate_component_reflection(
        reporter,
        objects,
        extension_contents_for_fqname,
        &mut imports,
    );
    let archetype_reflection = generate_archetype_reflection(reporter, objects);

    let mut code = format!("// {}\n\n", autogen_warning!());
    code.push_str("#![allow(clippy::allow_attributes)]\n");
    code.push_str("#![allow(clippy::empty_line_after_doc_comments)]\n");
    code.push_str("#![allow(clippy::too_many_lines)]\n");
    code.push_str("#![allow(clippy::wildcard_imports)]\n\n");
    code.push_str("#![allow(unused_imports)]\n");
    code.push('\n');
    for namespace in imports {
        code.push_str(&format!("use {namespace};\n"));
    }

    let quoted_reflection = quote! {
        use re_types_core::{
            ArchetypeName,
            ComponentType,
            Component,
            Loggable as _,
            ComponentBatch as _,
            reflection::{
                generate_component_identifier_reflection,
                ArchetypeFieldFlags,
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

            let archetypes = generate_archetype_reflection();

            Ok(Reflection {
                components: generate_component_reflection()?,
                component_identifiers: generate_component_identifier_reflection(&archetypes),
                archetypes
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
            quote!( ComponentType::new(#fqname) )
        };

        let docstring_md = doc_as_lines(
            reporter,
            objects,
            &obj.virtpath,
            &obj.fqname,
            &obj.state,
            &obj.docs,
            Target::WebDocsMarkdown,
        )
        .join("\n");

        // Emit custom placeholder if there's a default implementation
        let is_enum_with_default = obj.is_enum()
            && obj
                .fields
                .iter()
                .any(|field| field.attrs.has(crate::ATTR_DEFAULT));
        let has_default_attr = obj
            .try_get_attr::<String>(ATTR_RUST_DERIVE_ONLY)
            .or_else(|| obj.try_get_attr::<String>(ATTR_RUST_DERIVE))
            .is_some_and(|derives| derives.contains("Default"));
        let auto_derive_default = is_enum_with_default || has_default_attr;
        let has_custom_default_impl =
            extension_contents_for_fqname
                .get(&obj.fqname)
                .is_some_and(|contents| {
                    contents.contains(&format!("impl Default for {}", &obj.name))
                        || contents.contains(&format!("impl Default for super::{}", &obj.name))
                });
        let custom_placeholder = if auto_derive_default || has_custom_default_impl {
            quote! { Some(#type_name::default().to_arrow()?) }
        } else {
            quote! { None }
        };

        let deprecation_summary = if let Some(notice) = obj.deprecation_summary() {
            quote! { Some(#notice) }
        } else {
            quote! { None }
        };

        let is_enum = obj.is_enum();
        let quoted_reflection = quote! {
            ComponentReflection {
                docstring_md: #docstring_md,
                deprecation_summary: #deprecation_summary,
                custom_placeholder: #custom_placeholder,
                datatype: #type_name::arrow_datatype(),
                is_enum: #is_enum,
                verify_arrow_array: #type_name::verify_arrow_array,
            }
        };
        quoted_pairs.push(quote! { (#quoted_name, #quoted_reflection) });
    }

    quote! {
        #[doc = "Generates reflection about all known components."]
        #[doc = ""]
        #[doc = "Call only once and reuse the results."]
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
            let Some(component_type) = field.typ.fqname() else {
                reporter.error(
                    &field.virtpath,
                    &field.fqname,
                    "Archetype field must be an object/union or an array/vector of such",
                );
                return TokenStream::new();
            };
            let name = &field.name;
            let display_name = re_case::to_human_case(&field.name);
            let docstring_md = doc_as_lines(
                reporter,
                objects,
                &field.virtpath,
                &field.fqname,
                &field.state,
                &field.docs,
                Target::WebDocsMarkdown,
            )
            .join("\n");
            let required = field.attrs.has(ATTR_RERUN_COMPONENT_REQUIRED);
            let ui_editable = match field
                .try_get_attr::<String>(ATTR_RERUN_COMPONENT_UI_EDITABLE)
                .as_deref()
            {
                Some("true") => true,
                Some("false") => false,
                Some(value) => {
                    reporter.error(
                        &field.virtpath,
                        &field.fqname,
                        format!(
                            "Invalid value for {ATTR_RERUN_COMPONENT_UI_EDITABLE}: {value:?}. Expected \"true\" or \"false\"."
                        ),
                    );
                    !required
                }
                None => !required,
            };

            let mut flag_tokens: Vec<TokenStream> = Vec::new();
            if required {
                flag_tokens.push(quote! { ArchetypeFieldFlags::REQUIRED });
            }
            if ui_editable {
                flag_tokens.push(quote! { ArchetypeFieldFlags::UI_EDITABLE });
            }
            let flags = if flag_tokens.is_empty() {
                quote! { ArchetypeFieldFlags::empty() }
            } else {
                flag_tokens
                    .into_iter()
                    .reduce(|a, b| quote! { #a | #b })
                    .unwrap()
            };

            quote! {
                ArchetypeFieldReflection {
                    name: #name,
                    display_name: #display_name,
                    component_type: #component_type.into(),
                    docstring_md: #docstring_md,
                    flags: #flags,
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
                &obj.state,
                &obj.docs,
                Target::WebDocsMarkdown,
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

        let deprecation_summary = if let Some(notice) = obj.deprecation_summary() {
            quote! { Some(#notice) }
        } else {
            quote! { None }
        };

        let quoted_archetype_reflection = quote! {
            ArchetypeReflection {
                display_name: #display_name,

                deprecation_summary: #deprecation_summary,

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

/// Returns `crate_name` as is, unless it's `re_sdk_types`, in which case it's replace by `crate`,
/// because that's where this code lives.
fn patched_crate_name(crate_name: &str) -> String {
    if crate_name == "re_sdk_types" {
        "crate".to_owned()
    } else {
        crate_name.to_owned()
    }
}
