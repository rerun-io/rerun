use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;

use super::{ANGLE_BRACKET_LEFT_TOKEN, ANGLE_BRACKET_RIGHT_TOKEN, NEWLINE_TOKEN};
use crate::objects::is_testing_fqname;

/// Keeps track of necessary includes for a file.
pub struct Includes {
    system: BTreeSet<String>,
    local: BTreeSet<String>,
    fqname: String,
    scope: Option<String>,
}

impl Includes {
    pub fn new(fqname: String, scope: Option<String>) -> Self {
        Self {
            fqname,
            scope,
            system: BTreeSet::new(),
            local: BTreeSet::new(),
        }
    }

    /// `#include <vector>` etc
    pub fn insert_system(&mut self, name: &str) {
        self.system.insert(name.to_owned());
    }

    /// Insert a relative include path.
    pub fn insert_relative(&mut self, name: &str) {
        self.local.insert(name.to_owned());
    }

    /// Insert an include path that is in the `rerun` folder of the sdk.
    pub fn insert_rerun(&mut self, name: &str) {
        if is_testing_fqname(&self.fqname) {
            self.insert_system(&format!("rerun/{name}"));
        } else if self.scope.is_some() {
            self.local.insert(format!("../../{name}"));
        } else {
            self.local.insert(format!("../{name}"));
        }
    }

    /// Insert an include path to another generated type.
    pub fn insert_rerun_type(&mut self, included_fqname: &str) {
        let included_fqname_without_testing = included_fqname.replace(".testing", "");

        let components = included_fqname_without_testing
            .split('.')
            .collect::<Vec<_>>();

        let (path, typname) = match components[..] {
            ["rerun", obj_kind, typname] => (obj_kind.to_owned(), typname),
            ["rerun", scope, obj_kind, typname] => (format!("{scope}/{obj_kind}"), typname),
            _ => {
                panic!(
                    "Can't figure out include for {included_fqname:?} when adding includes for {:?}",
                    self.fqname
                );
            }
        };

        let typname = re_case::to_snake_case(typname);

        if is_testing_fqname(&self.fqname) == is_testing_fqname(included_fqname) {
            // If the type is in the same library, we use a relative path.
            if self
                .fqname
                .starts_with(&included_fqname[..included_fqname.len() - typname.len()])
            {
                // Types are next to each other, can skip going into the obj_kind folder.
                self.local.insert(format!("{typname}.hpp"));
            } else if self.scope.is_some() {
                self.local.insert(format!("../../{path}/{typname}.hpp"));
            } else {
                self.local.insert(format!("../{path}/{typname}.hpp"));
            }
        } else {
            // Types are not in the same library, need to treat this like a rerun sdk header.
            assert!(
                is_testing_fqname(&self.fqname) || !is_testing_fqname(included_fqname),
                "A non-testing type can't include a testing type."
            );
            self.insert_rerun(&format!("{path}/{typname}.hpp"));
        }
    }

    /// Remove all includes that are also in `other`.
    pub fn remove_includes(&mut self, other: &Self) {
        self.system.retain(|name| !other.system.contains(name));
        self.local.retain(|name| !other.local.contains(name));
    }
}

impl quote::ToTokens for Includes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            system,
            local,
            fqname: _,
            scope: _,
        } = self;

        let hash = quote! { # };
        let system = system.iter().map(|name| {
            // Need to mark system includes with tokens since they are usually not idents (can contain slashes and dots)
            quote! { #hash include #ANGLE_BRACKET_LEFT_TOKEN #name #ANGLE_BRACKET_RIGHT_TOKEN #NEWLINE_TOKEN }
        });
        let local = local.iter().map(|name| {
            quote! { #hash include #name #NEWLINE_TOKEN }
        });

        // Put the local includes first. This is less common but makes it easier for us to early detect
        // when a header relies on some system includes being present.
        // (all our headers should be standalone, i.e. don't assume something else was included before them)
        quote! {
            #(#local)*
            #NEWLINE_TOKEN
            #(#system)*
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
        }
        .to_tokens(tokens);
    }
}
