//! Generates the module tree that makes the type definitions a real Rust crate.
//!
//! The definitions are a subset of Rust that `re_types_builder` parses (see
//! `objects/from_rust.rs`), but they are *also* compiled by rustc — that is what buys us
//! name resolution, typo-checking, rust-analyzer and `cargo fmt`. Nothing links the resulting
//! crate; it is built for its diagnostics alone.
//!
//! For rustc to see a file it has to be declared, so every directory needs a module file listing
//! its contents. Those files hold no type definitions; they are generated from the directory
//! listing, so that adding a definition is just adding a file.
//!
//! The crate root is `rerun/lib.rs`, so that `rerun/components/position3d.rs` is the module
//! `crate::components::position3d`, and the `extern crate self as rerun;` in the root makes
//! `rerun::components::Position3D` — the fully-qualified name with `::` for `.` — resolve from
//! anywhere, with no `use` statement in any definition.

use std::fmt::Write as _;

use camino::{Utf8Path, Utf8PathBuf};

use super::autogen_warning;
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
        reporter: &Reporter,
        _objects: &crate::Objects, // Generated from the directory listing.
        _type_registry: &crate::TypeRegistry, // Ditto.
    ) -> GeneratedFiles {
        let mut files = GeneratedFiles::default();
        let crate_root = self.definitions_dir.join(CRATE_ROOT_DIR);
        generate_module_files(reporter, &crate_root, &crate_root, &mut files);
        files
    }
}

/// Emits the module file for `dir` and, recursively, for every directory below it.
fn generate_module_files(
    reporter: &Reporter,
    crate_root: &Utf8Path,
    dir: &Utf8Path,
    files: &mut GeneratedFiles,
) {
    let Some(contents) = read_dir(reporter, dir) else {
        return;
    };

    let is_crate_root = dir == crate_root;
    let path = if is_crate_root {
        dir.join("lib.rs")
    } else {
        // `foo/` is declared by `foo.rs` sitting next to it.
        dir.with_extension("rs")
    };

    files.insert(path, module_file(&contents, is_crate_root));

    for subdir in contents.subdirectories {
        generate_module_files(reporter, crate_root, &dir.join(subdir), files);
    }
}

/// The definitions and sub-packages of a single directory, sorted.
struct DirContents {
    /// Module names of the definition files, e.g. `position3d`.
    definitions: Vec<String>,

    /// Directory names of the sub-packages, e.g. `components`.
    subdirectories: Vec<String>,
}

fn read_dir(reporter: &Reporter, dir: &Utf8Path) -> Option<DirContents> {
    let entries = match dir.read_dir_utf8() {
        Ok(entries) => entries,
        Err(err) => {
            reporter.error_file(dir, err);
            return None;
        }
    };

    let mut definitions = Vec::new();
    let mut subdirectories = Vec::new();

    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.into_path(),
            Err(err) => {
                reporter.error_file(dir, err);
                return None;
            }
        };

        let Some(name) = path.file_stem() else {
            continue;
        };

        if path.is_dir() {
            subdirectories.push(name.to_owned());
        } else if path.extension() == Some("rs") {
            definitions.push(name.to_owned());
        }
    }

    definitions.sort();
    subdirectories.sort();

    // A module file is not a definition, and would otherwise declare itself.
    definitions.retain(|name| name != "lib" && !subdirectories.contains(name));

    // `mod` is a keyword, so `mod mod;` does not even compile. The tree declares a directory with a
    // `foo.rs` next to it, in the style rustc has preferred since the 2018 edition.
    if let Some(index) = definitions.iter().position(|name| name == "mod") {
        definitions.remove(index);
        reporter.error_file(
            &dir.join("mod.rs"),
            "A definition file cannot be called `mod.rs`; a directory `foo/` is declared by a \
             `foo.rs` next to it",
        );
    }

    Some(DirContents {
        definitions,
        subdirectories,
    })
}

fn module_file(contents: &DirContents, is_crate_root: bool) -> String {
    let DirContents {
        definitions,
        subdirectories,
    } = contents;

    let mut out = format!("// {}\n", autogen_warning!());

    if is_crate_root {
        write!(
            out,
            "
// Rerun's type definitions. This crate is never linked into anything: rustc compiles it so that
// we get name resolution, typo-checking, rust-analyzer and `cargo fmt` on the definitions.
//
// `extern crate self as rerun;` is what lets a definition refer to another one by its
// fully-qualified name — `rerun::components::Position3D` — with no `use` statement anywhere.

extern crate self as rerun;

// Everything a definition may name that no definition declares: the types with no spelling in
// plain Rust, and the attribute macro. Glob-imported, so that the vocabulary lives in the prelude
// alone and this generator does not have to know it.
pub use {PRELUDE}::*;
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
        out.push('\n');
        for name in definitions {
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
        let generated = module_file(&contents(&["position3d", "radius"], &[]), false);
        insta::assert_snapshot!(generated, @r"
        // DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/definitions/mod.rs

        mod position3d;
        mod radius;

        pub use self::position3d::*;
        pub use self::radius::*;
        ");
    }

    #[test]
    fn crate_root_module_file() {
        let generated = module_file(
            &contents(&[], &["blueprint", "components", "datatypes"]),
            true,
        );
        insta::assert_snapshot!(generated, @r#"
        // DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/definitions/mod.rs

        // Rerun's type definitions. This crate is never linked into anything: rustc compiles it so that
        // we get name resolution, typo-checking, rust-analyzer and `cargo fmt` on the definitions.
        //
        // `extern crate self as rerun;` is what lets a definition refer to another one by its
        // fully-qualified name — `rerun::components::Position3D` — with no `use` statement anywhere.

        extern crate self as rerun;

        // Everything a definition may name that no definition declares: the types with no spelling in
        // plain Rust, and the attribute macro. Glob-imported, so that the vocabulary lives in the prelude
        // alone and this generator does not have to know it.
        pub use re_types_builder_prelude::*;

        pub mod blueprint;
        pub mod components;
        pub mod datatypes;
        "#);
    }

    #[test]
    fn a_directory_can_hold_both_definitions_and_sub_packages() {
        let generated = module_file(&contents(&["type_zoo"], &["archetypes"]), false);
        insta::assert_snapshot!(generated, @r"
        // DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/definitions/mod.rs

        pub mod archetypes;

        mod type_zoo;

        pub use self::type_zoo::*;
        ");
    }
}
