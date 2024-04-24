//! Generates code in `re_query` so that cached results can easily be converted to
//! ready-to-use archetypes.
//!
//! That code needs to be generated directly in the caching crates as it needs access to the cached
//! queries and results as well as the promise resolving machinery.
//! Generating such code in the usual places would result in one giant cycle dependency chain.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{objects::FieldKind, Object, ObjectKind, Objects, Reporter};

// ---

const NEWLINE_TOKEN: &str = "NEWLINE_TOKEN";
const COMMENT_SEPARATOR_TOKEN: &str = "COMMENT_SEPARATOR_TOKEN";
const COMMENT_REQUIRED_TOKEN: &str = "COMMENT_REQUIRED_TOKEN";
const COMMENT_RECOMMENDED_OPTIONAL_TOKEN: &str = "COMMENT_RECOMMENDED_OPTIONAL_TOKEN";

pub fn generate_to_archetype_impls(
    reporter: &Reporter,
    objects: &Objects,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    generate_mod(reporter, objects, files_to_write);
    generate_impls(reporter, objects, files_to_write);
}

fn generate_mod(
    _reporter: &Reporter,
    objects: &Objects,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    let generated_path = Utf8PathBuf::from("crates/re_query/src/latest_at/to_archetype/mod.rs");

    let mut code = String::new();
    code.push_str(&format!("// {}\n\n", crate::codegen::autogen_warning!()));

    let mut mods = Vec::new();

    for obj in objects.ordered_objects(Some(ObjectKind::Archetype)) {
        // TODO(#4478): add a 'testing' scope
        if obj.fqname.contains("testing") {
            continue;
        }

        let arch_name = obj.snake_case_name();
        mods.push(format!("mod {arch_name};"));
    }

    code.push_str(&mods.join("\n"));

    files_to_write.insert(generated_path, code);
}

fn generate_impls(
    _reporter: &Reporter,
    objects: &Objects,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    let generated_path = Utf8PathBuf::from("crates/re_query/src/latest_at/to_archetype");

    let quoted_imports = quote! {
        use std::sync::Arc;

        use re_types_core::{Archetype, Loggable as _};

        use crate::{CachedLatestAtResults, PromiseResolver, PromiseResult};
    };

    for obj in objects.ordered_objects(Some(ObjectKind::Archetype)) {
        if obj
            .try_get_attr::<String>(crate::ATTR_RUST_SERDE_TYPE)
            .is_some()
        {
            // NOTE: legacy serde-based hacks.
            continue;
        }

        // TODO(#4478): add a 'testing' scope
        if obj.fqname.contains("testing") {
            continue;
        }

        let quoted_imports = quoted_imports.to_string();
        let quoted_impl = quote_to_archetype_impl(objects, obj);

        let mut code = String::new();
        code.push_str(&format!("// {}\n\n", crate::codegen::autogen_warning!()));
        code.push_str("#![allow(unused_imports)]\n");
        code.push_str("#![allow(unused_parens)]\n");
        code.push_str("#![allow(clippy::clone_on_copy)]\n");
        code.push_str("#![allow(clippy::cloned_instead_of_copied)]\n");
        if obj.deprecation_notice().is_some() {
            code.push_str("#![allow(deprecated)]\n");
        }
        code.push_str(&format!("\n\n{quoted_imports}\n\n"));
        code.push_str(&quoted_impl.to_string());

        let arch_name = obj.snake_case_name();
        files_to_write.insert(
            generated_path.join([arch_name.as_str(), "rs"].join(".")),
            code.replace(&format!("{NEWLINE_TOKEN:?}"), "\n\n")
                .replace(&format!("{COMMENT_SEPARATOR_TOKEN:?}"), "\n\n// --- \n\n")
                .replace(
                    &format!("{COMMENT_REQUIRED_TOKEN:?}"),
                    "\n\n// --- Required ---\n\n",
                )
                .replace(
                    &format!("{COMMENT_RECOMMENDED_OPTIONAL_TOKEN:?}"),
                    "\n\n// --- Recommended/Optional ---\n\n",
                ),
        );
    }
}

fn quote_to_archetype_impl(objects: &Objects, obj: &Object) -> TokenStream {
    assert!(obj.kind == ObjectKind::Archetype);

    let quoted_arch_fqname = quote_fqname_as_type_path(&obj.crate_name(), &obj.fqname);

    let quoted_required = obj
        .fields
        .iter()
        .filter(|obj_field| obj_field.kind() == Some(FieldKind::Required))
        .filter_map(|obj_field| {
            let quoted_name = format_ident!("{}", obj_field.name);

            let type_fqname = obj_field.typ.fqname()?;
            let type_name = type_fqname.rsplit_once('.').map(|(_, name)| name)?;

            let quoted_type_name = format_ident!("{type_name}");
            let quoted_type_fqname =
                quote_fqname_as_type_path(&objects[type_fqname].crate_name(), type_fqname);

            let quoted_data = if obj_field.typ.is_plural() {
                quote!(data.to_vec())
            } else {
                quote! {{
                    let Some(first) = data.first().cloned() else {
                        return PromiseResult::Error(
                            std::sync::Arc::new(re_types_core::DeserializationError::missing_data())
                        );
                    };
                    first
                }}
            };

            Some(quote! {
                #NEWLINE_TOKEN

                use #quoted_type_fqname;
                let #quoted_name = match self.get_required(<#quoted_type_name>::name()) {
                    Ok(#quoted_name) => #quoted_name,
                    Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                };
                let #quoted_name = match #quoted_name.to_dense::<#quoted_type_name>(resolver) {
                    PromiseResult::Pending => return PromiseResult::Pending,
                    PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                    PromiseResult::Ready(query_res) => match query_res {
                        Ok(data) => #quoted_data,
                        Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                    },
                };
            })
        });

    let quoted_optional = obj
        .fields
        .iter()
        .filter(|obj_field| obj_field.kind() != Some(FieldKind::Required))
        .filter_map(|obj_field| {
            let quoted_name = format_ident!("{}", obj_field.name);

            let type_fqname = obj_field.typ.fqname()?;
            let type_name = type_fqname.rsplit_once('.').map(|(_, name)| name)?;

            let quoted_type_name = format_ident!("{type_name}");
            let quoted_type_fqname =
            quote_fqname_as_type_path(&objects[type_fqname].crate_name(), type_fqname);

            if obj_field.is_nullable {
                let quoted_data = if obj_field.typ.is_plural() {
                    quote!(Some(data.to_vec()))
                } else {
                    quote!(data.first().cloned())
                };

                Some(quote! {
                    #NEWLINE_TOKEN

                    use #quoted_type_fqname;
                    let #quoted_name = if let Some(#quoted_name) = self.get(<#quoted_type_name>::name()) {
                        match #quoted_name.to_dense::<#quoted_type_name>(resolver) {
                            PromiseResult::Pending => return PromiseResult::Pending,
                            PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                            PromiseResult::Ready(query_res) => match query_res {
                                Ok(data) => #quoted_data,
                                Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                            },
                        }
                    } else {
                        None
                    };
                })
            } else {
                let quoted_data = if obj_field.typ.is_plural() {
                    quote!(data.to_vec())
                } else {
                    panic!("optional, non-nullable, non-plural data is not representable");
                };

                Some(quote! {
                    #NEWLINE_TOKEN

                    use #quoted_type_fqname;
                    let #quoted_name =
                        match self.get_or_empty(<#quoted_type_name>::name()).to_dense::<#quoted_type_name>(resolver) {
                            PromiseResult::Pending => return PromiseResult::Pending,
                            PromiseResult::Error(promise_err) => return PromiseResult::Error(promise_err),
                            PromiseResult::Ready(query_res) => match query_res {
                                Ok(data) => #quoted_data,
                                Err(query_err) => return PromiseResult::Ready(Err(query_err)),
                            },
                        };
                })
            }

        });

    let quoted_fields = obj.fields.iter().map(|obj_field| {
        let quoted_name = format_ident!("{}", obj_field.name);
        quote!(#quoted_name)
    });

    quote! {
        impl crate::ToArchetype<#quoted_arch_fqname> for CachedLatestAtResults {
            #[inline]
            fn to_archetype(
                &self,
                resolver: &PromiseResolver,
            ) -> PromiseResult<crate::Result<#quoted_arch_fqname>> {
                #NEWLINE_TOKEN
                re_tracing::profile_function!(<#quoted_arch_fqname>::name());
                #NEWLINE_TOKEN

                // --- Required ---
                #COMMENT_REQUIRED_TOKEN

                #(#quoted_required)*

                // --- Recommended/Optional ---
                #COMMENT_RECOMMENDED_OPTIONAL_TOKEN

                #(#quoted_optional)*

                // ---
                #COMMENT_SEPARATOR_TOKEN

                // TODO(cmc): A lot of useless copying going on since archetypes are fully owned
                // types. Probably fine for now since these are very high-level APIs anyhow.
                let arch = #quoted_arch_fqname {
                    #(#quoted_fields),*
                };

                #NEWLINE_TOKEN

                PromiseResult::Ready(Ok(arch))
            }
        }
    }
}

// ---

fn quote_fqname_as_type_path(crate_name: &str, fqname: impl AsRef<str>) -> TokenStream {
    let fqname = fqname
        .as_ref()
        .replace('.', "::")
        .replace("rerun", crate_name);
    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}
