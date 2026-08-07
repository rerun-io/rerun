//! Generates the module tree that makes the type definitions a real Rust crate.
//!
//! The definitions are a subset of Rust that `re_types_builder` parses (see
//! `objects/from_rust.rs`), but they are *also* compiled by rustc — that is what buys us
//! name resolution, typo-checking, rust-analyzer and `cargo fmt`. Nothing links the resulting
//! crate; it is built for its diagnostics alone.
//!
//! For rustc to see a file it has to be declared, so every directory needs a module file listing
//! its contents. Those files hold no type definitions; they are generated from the definitions
//! themselves, so that adding a definition is just adding a file.
//!
//! A definition is named `position3d.def.rs`, which is not a file name rustc would look for, so
//! every one of them is declared with an explicit `#[path]`. The name is what tells a definition
//! apart from the module tree it sits in, and from every other `.rs` file in the repo.
//!
//! The crate root is `rerun/lib.rs`, so that `rerun/components/position3d.def.rs` is the module
//! `crate::components::position3d`, and the `extern crate self as rerun;` in the root makes
//! `rerun::components::Position3D` — the fully-qualified name with `::` for `.` — resolve from
//! anywhere, with no `use` statement in any definition.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use camino::{Utf8Path, Utf8PathBuf};

use super::autogen_warning;
use crate::objects::from_rust::{DEFINITION_SUFFIX, definition_module_name};
use crate::{CodeGenerator, GeneratedFiles, Reporter};

/// The directory holding the definition tree, relative to the definitions root.
///
/// This is the crate root's directory, not the definitions root, so that the module path of a
/// definition matches its fully-qualified name.
const CRATE_ROOT_DIR: &str = "rerun";

/// What every definition module file re-exports from, so that a type is addressed by its package
/// rather than by the file it happens to live in.
const PRELUDE: &str = "re_types_builder_prelude";

/// Writes the `mod` / `pub use` scaffolding that declares the definitions to rustc.
///
/// It generates no type definitions of its own: it reads which files hold them and emits one
/// module file per directory, so that adding a definition is just adding a file.
pub struct DefinitionsCodeGenerator {
    definitions_dir: Utf8PathBuf,
}

impl DefinitionsCodeGenerator {
    pub fn new(definitions_dir: impl Into<Utf8PathBuf>) -> Self {
        Self {
            definitions_dir: definitions_dir.into(),
        }
    }
}

impl CodeGenerator for DefinitionsCodeGenerator {
    fn generate(
        &mut self,
        _reporter: &Reporter,
        objects: &crate::Objects,
        _type_registry: &crate::TypeRegistry, // The definitions are the input to all of that.
    ) -> GeneratedFiles {
        // Several types can share a file, and a file is declared once.
        let definition_paths: BTreeSet<Utf8PathBuf> = objects
            .objects
            .values()
            .map(|object| object.filepath.clone())
            .collect();

        let mut files = GeneratedFiles::default();

        let crate_root = self.definitions_dir.join(CRATE_ROOT_DIR);
        let mut module_files = Vec::new();
        for (dir, contents) in directory_tree(&crate_root, definition_paths.iter()) {
            // The crate root is `lib.rs` inside its directory; every other `foo/` is declared by
            // the `foo.rs` sitting next to it.
            let (path, module_dir) = if dir == crate_root {
                (dir.join("lib.rs"), None)
            } else {
                (dir.with_extension("rs"), dir.file_name())
            };
            module_files.push(path.clone());
            files.insert(path, module_file(&contents, module_dir));
        }

        // The module tree is ours; the definitions themselves are written by hand.
        crate::mark_as_generated(&mut files, module_files);

        files
    }
}

/// The definitions and sub-packages of a single directory.
#[derive(Default)]
struct DirContents {
    /// Module names of the definition files, e.g. `position3d`.
    definitions: BTreeSet<String>,

    /// Directory names of the sub-packages, e.g. `components`.
    subdirectories: BTreeSet<String>,
}

/// Every directory from `crate_root` down, and what it holds.
fn directory_tree<'a>(
    crate_root: &Utf8Path,
    definition_paths: impl Iterator<Item = &'a Utf8PathBuf>,
) -> BTreeMap<Utf8PathBuf, DirContents> {
    // The crate root always gets a module file, even if every definition is in a sub-package.
    let mut dirs = BTreeMap::from([(crate_root.to_owned(), DirContents::default())]);

    for path in definition_paths {
        let (Some(dir), Some(module)) = (path.parent(), definition_module_name(path)) else {
            continue;
        };

        dirs.entry(dir.to_owned())
            .or_default()
            .definitions
            .insert(module.to_owned());

        // Every directory on the way to the root has to declare the one below it.
        let mut child = dir;
        while child != crate_root {
            let (Some(parent), Some(name)) = (child.parent(), child.file_name()) else {
                break;
            };
            dirs.entry(parent.to_owned())
                .or_default()
                .subdirectories
                .insert(name.to_owned());
            child = parent;
        }
    }

    dirs
}

/// Writes the module file for one directory.
///
/// `module_dir` is the name of the directory the file declares — `components` for
/// `rerun/components.rs` — and `None` for the crate root, which is `lib.rs` inside its own
/// directory.
fn module_file(contents: &DirContents, module_dir: Option<&str>) -> String {
    let DirContents {
        definitions,
        subdirectories,
    } = contents;

    let mut out = format!("// {}\n", autogen_warning!());

    if module_dir.is_none() {
        write!(
            out,
            "
// Rerun's type definitions. This crate is never linked into anything: rustc compiles it so that
// we get name resolution, typo-checking, rust-analyzer and `cargo fmt` on the definitions.
//
// `extern crate self as rerun;` is what lets a definition refer to another one by its
// fully-qualified name — `rerun::components::Position3D` — with no `use` statement anywhere.

// Nothing links this crate. A type's name is API and wire format — spelled the way the SDKs
// spell it, not the way Rust would — and its docstring is written for the SDK docs.
#![allow(clippy::doc_markdown)]
#![allow(clippy::upper_case_acronyms)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(rustdoc::all)]

extern crate self as rerun;

// The two types that have no spelling in plain Rust, re-exported here so that the
// `rerun::`-rooted rule holds without exceptions.
pub use {PRELUDE}::{{Binary, f16, rerun_type}};
"
        )
        .ok();
    }

    // Sub-packages are `pub` so that they can be named from other definitions; the definition
    // files themselves are private and re-exported below, so that a type is addressed by its
    // package and not by the file it happens to live in.
    if !subdirectories.is_empty() {
        out.push('\n');
        for name in subdirectories {
            writeln!(out, "pub mod {name};").ok();
        }
    }

    if !definitions.is_empty() {
        // `position3d.def.rs` is not a file name rustc would look for, hence the `#[path]`. It is
        // resolved relative to the directory holding *this* file, which is one above the
        // definitions, so the directory has to be spelled out.
        let prefix = module_dir.map(|dir| format!("{dir}/")).unwrap_or_default();

        out.push('\n');
        for name in definitions {
            writeln!(out, "#[path = \"{prefix}{name}{DEFINITION_SUFFIX}\"]").ok();
            writeln!(out, "mod {name};").ok();
        }

        out.push('\n');
        for name in definitions {
            writeln!(out, "pub use self::{name}::*;").ok();
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents(definitions: &[&str], subdirectories: &[&str]) -> DirContents {
        DirContents {
            definitions: definitions.iter().map(|s| (*s).to_owned()).collect(),
            subdirectories: subdirectories.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn package_module_file() {
        let generated = module_file(
            &contents(&["position3d", "radius"], &[]),
            Some("components"),
        );
        insta::assert_snapshot!(generated, @r##"
        // DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/definitions/mod.rs

        #[path = "components/position3d.def.rs"]
        mod position3d;
        #[path = "components/radius.def.rs"]
        mod radius;

        pub use self::position3d::*;
        pub use self::radius::*;
        "##);
    }

    #[test]
    fn crate_root_module_file() {
        let generated = module_file(
            &contents(&[], &["blueprint", "components", "datatypes"]),
            None,
        );
        insta::assert_snapshot!(generated, @r#"
        // DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/definitions/mod.rs

        // Rerun's type definitions. This crate is never linked into anything: rustc compiles it so that
        // we get name resolution, typo-checking, rust-analyzer and `cargo fmt` on the definitions.
        //
        // `extern crate self as rerun;` is what lets a definition refer to another one by its
        // fully-qualified name — `rerun::components::Position3D` — with no `use` statement anywhere.

        // Nothing links this crate. A type's name is API and wire format — spelled the way the SDKs
        // spell it, not the way Rust would — and its docstring is written for the SDK docs.
        #![allow(clippy::doc_markdown)]
        #![allow(clippy::upper_case_acronyms)]
        #![allow(dead_code)]
        #![allow(non_camel_case_types)]
        #![allow(rustdoc::all)]

        extern crate self as rerun;

        // The two types that have no spelling in plain Rust, re-exported here so that the
        // `rerun::`-rooted rule holds without exceptions.
        pub use re_types_builder_prelude::{Binary, f16, rerun_type};

        pub mod blueprint;
        pub mod components;
        pub mod datatypes;
        "#);
    }

    #[test]
    fn a_directory_can_hold_both_definitions_and_sub_packages() {
        let generated = module_file(&contents(&["type_zoo"], &["archetypes"]), Some("testing"));
        insta::assert_snapshot!(generated, @r##"
        // DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/definitions/mod.rs

        pub mod archetypes;

        #[path = "testing/type_zoo.def.rs"]
        mod type_zoo;

        pub use self::type_zoo::*;
        "##);
    }

    #[test]
    fn the_module_tree_follows_the_definitions() {
        let crate_root = Utf8Path::new("definitions/rerun");
        let paths = [
            Utf8PathBuf::from("definitions/rerun/blueprint/archetypes/background.def.rs"),
            Utf8PathBuf::from("definitions/rerun/blueprint/views/spatial3d.def.rs"),
            Utf8PathBuf::from("definitions/rerun/components/position3d.def.rs"),
            Utf8PathBuf::from("definitions/rerun/components/radius.def.rs"),
        ];

        let tree = directory_tree(crate_root, paths.iter());

        let summary = tree
            .iter()
            .map(|(dir, contents)| {
                format!(
                    "{dir}: [{}] [{}]",
                    contents
                        .subdirectories
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                    contents
                        .definitions
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        insta::assert_snapshot!(summary, @r"
        definitions/rerun: [blueprint, components] []
        definitions/rerun/blueprint: [archetypes, views] []
        definitions/rerun/blueprint/archetypes: [] [background]
        definitions/rerun/blueprint/views: [] [spatial3d]
        definitions/rerun/components: [] [position3d, radius]
        ");
    }
}
