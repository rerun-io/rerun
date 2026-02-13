use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::util::string_from_quoted;
use crate::codegen::autogen_warning;
use crate::codegen::common::StringExt as _;
use crate::{Object, ObjectKind, Objects, Reporter};

pub(crate) fn generate_blueprint_validation(
    reporter: &Reporter,
    objects: &Objects,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    let blueprint_scope = Some("blueprint".to_owned());
    let mut code = String::new();
    code.push_str(&format!("// {}\n\n", autogen_warning!()));
    code.push_str("#![allow(clippy::empty_line_after_doc_comments)]\n\n");

    code.push_str("use re_entity_db::EntityDb;\n");
    code.push_str("use super::validation::validate_component;\n");

    for obj in objects.objects_of_kind(ObjectKind::Component) {
        if obj.scope() == blueprint_scope {
            let type_name = &obj.name;
            let mut crate_name = obj.crate_name();
            if crate_name == "re_viewer" {
                crate_name = "crate".to_owned();
            }
            code.push_str(&format!(
                "pub use {crate_name}::blueprint::components::{type_name};\n"
            ));
        }
    }

    let mut validations = TokenStream::new();
    let mut first = true;
    for obj in objects.objects_of_kind(ObjectKind::Component) {
        if obj.scope() == blueprint_scope {
            validations.extend(quote_component_validation(obj, first));
            first = false;
        }
    }

    let is_valid_blueprint = quote! {
        /// Because blueprints are both read and written the schema must match what
        /// we expect to find or else we will run into all kinds of problems.
        pub fn is_valid_blueprint(blueprint: &EntityDb) -> bool {
            #validations
        }
    };

    let path = Utf8PathBuf::from("crates/viewer/re_viewer/src/blueprint/validation_gen/mod.rs");
    code.push_indented(
        0,
        string_from_quoted(reporter, &is_valid_blueprint, &path),
        1,
    );

    files_to_write.insert(path, code);
}

fn quote_component_validation(obj: &Object, first: bool) -> TokenStream {
    let name = format_ident!("{}", obj.name);
    let quoted_join = if first {
        quote! {}
    } else {
        quote! {&&}
    };
    quote! {
        #quoted_join validate_component::<#name>(blueprint)
    }
}
