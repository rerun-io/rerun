//! This crate implements Rerun's code generation tools.
//!
//! These tools translate language-agnostic IDL definitions (flatbuffers) into code.
//! They are invoked by `re_types`'s build script (`build.rs`).
//!
//!
//! ### Organization
//!
//! The code generation process happens in 4 phases.
//!
//! #### 1. Generate binary reflection data from flatbuffers definitions.
//!
//! All this does is invoke the flatbuffers compiler (`flatc`) with the right flags in order to
//! generate the binary dumps.
//!
//! Look for `compile_binary_schemas` in the code.
//!
//! #### 2. Run the semantic pass.
//!
//! The semantic pass transforms the low-level raw reflection data generated by the first phase
//! into higher level objects that are much easier to inspect/manipulate and overall friendlier
//! to work with.
//!
//! Look for `objects.rs`.
//!
//! #### 3. Fill the Arrow registry.
//!
//! The Arrow registry keeps track of all type definitions and maps them to Arrow datatypes.
//!
//! Look for `arrow_registry.rs`.
//!
//! #### 4. Run the actual codegen pass for a given language.
//!
//! We currently have two different codegen passes implemented at the moment: Python & Rust.
//!
//! Codegen passes use the semantic objects from phase two and the registry from phase three
//! in order to generate user-facing code for Rerun's SDKs.
//!
//! These passes are intentionally implemented using a very low-tech no-frills approach (stitch
//! strings together, make liberal use of `unimplemented`, etc) that keep them flexible in the
//! face of ever changing needs in the generated code.
//!
//! Look for `codegen/python.rs` and `codegen/rust.rs`.
//!
//!
//! ### Error handling
//!
//! Keep in mind: this is all _build-time_ code that will never see the light of runtime.
//! There is therefore no need for fancy error handling in this crate: all errors are fatal to the
//! build anyway.
//!
//! Make sure to crash as soon as possible when something goes wrong and to attach all the
//! appropriate/available context using `anyhow`'s `with_context` (e.g. always include the
//! fully-qualified name of the faulty type/field) and you're good to go.
//!
//!
//! ### Testing
//!
//! Same comment as with error handling: this code becomes irrelevant at runtime, and so testing it
//! brings very little value.
//!
//! Make sure to test the behavior of its output though: `re_types`!
//!
//!
//! ### Understanding the subtleties of affixes
//!
//! So-called "affixes" are effects applied to objects defined with the Rerun IDL and that affect
//! the way these objects behave and interoperate with each other (so, yes, monads. shhh.).
//!
//! There are 3 distinct and very common affixes used when working with Rerun's IDL: transparency,
//! nullability and plurality.
//!
//! Broadly, we can describe these affixes as follows:
//! - Transparency allows for bypassing a single layer of typing (e.g. to "extract" a field out of
//!   a struct).
//! - Nullability specifies whether a piece of data is allowed to be left unspecified at runtime.
//! - Plurality specifies whether a piece of data is actually a collection of that same type.
//!
//! We say "broadly" here because the way these affixes ultimately affect objects in practice will
//! actually depend on the kind of object that they are applied to, of which there are 3: archetypes,
//! components and datatypes.
//!
//! Not only that, but objects defined in Rerun's IDL are materialized into 3 distinct environments:
//! IDL definitions, Arrow datatypes and native code (e.g. Rust & Python).
//!
//! These environment have vastly different characteristics, quirks, pitfalls and limitations,
//! which once again lead to these affixes having different, sometimes surprising behavior
//! depending on the environment we're interested in.
//! Also keep in mind that Flatbuffers and native code are generally designed around arrays of
//! structures, while Arrow is all about structures of arrays!
//!
//! All in all, these interactions between affixes, object kinds and environments lead to a
//! combinatorial explosion of edge cases that can be very confusing when it comes to (de)serialization
//! code, and even API design.
//!
//! When in doubt, check out the `rerun.testing.archetypes.AffixFuzzer` IDL definitions, generated code and
//! test suites for definitive answers.

// TODO(#2365): support for external IDL definitions

// ---

// NOTE: Official generated code from flatbuffers; ignore _everything_.
#[allow(
    warnings,
    unused,
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    dead_code,
    unused_imports,
    explicit_outlives_requirements,
    clippy::all
)]
mod reflection;

use std::collections::BTreeMap;

use anyhow::Context as _;
use re_build_tools::{
    compute_crate_hash, compute_dir_filtered_hash, compute_dir_hash, compute_strings_hash,
};

pub use self::reflection::reflection::{
    root_as_schema, BaseType as FbsBaseType, Enum as FbsEnum, EnumVal as FbsEnumVal,
    Field as FbsField, KeyValue as FbsKeyValue, Object as FbsObject, Schema as FbsSchema,
    Type as FbsType,
};

// NOTE: This crate isn't only okay with `unimplemented`, it actively encourages it.

#[allow(clippy::unimplemented)]
mod arrow_registry;
#[allow(clippy::unimplemented)]
mod codegen;
#[allow(clippy::unimplemented)]
mod objects;
pub mod report;

pub use self::arrow_registry::{ArrowRegistry, LazyDatatype, LazyField};
pub use self::codegen::{
    CodeGenerator, CppCodeGenerator, DocsCodeGenerator, GeneratedFiles, PythonCodeGenerator,
    RustCodeGenerator,
};
pub use self::objects::{
    Attributes, Docs, ElementType, Object, ObjectField, ObjectKind, ObjectSpecifics, Objects, Type,
};
pub use self::report::{Report, Reporter};

// --- Attributes ---

pub const ATTR_NULLABLE: &str = "nullable";
pub const ATTR_ORDER: &str = "order";
pub const ATTR_TRANSPARENT: &str = "transparent";

pub const ATTR_ARROW_TRANSPARENT: &str = "attr.arrow.transparent";
pub const ATTR_ARROW_SPARSE_UNION: &str = "attr.arrow.sparse_union";

pub const ATTR_RERUN_COMPONENT_OPTIONAL: &str = "attr.rerun.component_optional";
pub const ATTR_RERUN_COMPONENT_RECOMMENDED: &str = "attr.rerun.component_recommended";
pub const ATTR_RERUN_COMPONENT_REQUIRED: &str = "attr.rerun.component_required";
pub const ATTR_RERUN_OVERRIDE_TYPE: &str = "attr.rerun.override_type";

pub const ATTR_PYTHON_ALIASES: &str = "attr.python.aliases";
pub const ATTR_PYTHON_ARRAY_ALIASES: &str = "attr.python.array_aliases";

pub const ATTR_RUST_CUSTOM_CLAUSE: &str = "attr.rust.custom_clause";
pub const ATTR_RUST_DERIVE: &str = "attr.rust.derive";
pub const ATTR_RUST_DERIVE_ONLY: &str = "attr.rust.derive_only";
pub const ATTR_RUST_NEW_PUB_CRATE: &str = "attr.rust.new_pub_crate";
pub const ATTR_RUST_OVERRIDE_CRATE: &str = "attr.rust.override_crate";
pub const ATTR_RUST_REPR: &str = "attr.rust.repr";
pub const ATTR_RUST_SERDE_TYPE: &str = "attr.rust.serde_type";
pub const ATTR_RUST_TUPLE_STRUCT: &str = "attr.rust.tuple_struct";

pub const ATTR_CPP_NO_FIELD_CTORS: &str = "attr.cpp.no_field_ctors";

// --- Entrypoints ---

use camino::{Utf8Path, Utf8PathBuf};

/// Compiles binary reflection dumps from flatbuffers definitions.
///
/// Requires `flatc` available in $PATH.
///
/// Panics on error.
///
/// - `include_dir_path`: path to the root directory of the fbs definition tree.
/// - `output_dir_path`: output directory, where the binary schemas will be stored.
/// - `entrypoint_path`: path to the root file of the fbs definition tree.
///
/// E.g.:
/// ```no_run
/// re_types_builder::compile_binary_schemas(
///     "definitions/",
///     "out/",
///     "definitions/rerun/archetypes.fbs",
/// );
/// ```
pub fn compile_binary_schemas(
    include_dir_path: impl AsRef<Utf8Path>,
    output_dir_path: impl AsRef<Utf8Path>,
    entrypoint_path: impl AsRef<Utf8Path>,
) {
    let include_dir_path = include_dir_path.as_ref().as_str();
    let output_dir_path = output_dir_path.as_ref().as_str();
    let entrypoint_path = entrypoint_path.as_ref().as_str();

    use xshell::{cmd, Shell};
    let sh = Shell::new().unwrap();
    cmd!(
        sh,
        "flatc -I {include_dir_path}
            -o {output_dir_path}
            -b --bfbs-comments --schema
            {entrypoint_path}"
    )
    .run()
    .unwrap();
}

/// Handles the first 3 language-agnostic passes of the codegen pipeline:
/// 1. Generate binary reflection dumps for our definitions.
/// 2. Run the semantic pass
/// 3. Compute the Arrow registry
///
/// Panics on error.
///
/// - `include_dir_path`: path to the root directory of the fbs definition tree.
/// - `entrypoint_path`: path to the root file of the fbs definition tree.
pub fn generate_lang_agnostic(
    include_dir_path: impl AsRef<Utf8Path>,
    entrypoint_path: impl AsRef<Utf8Path>,
) -> (Objects, ArrowRegistry) {
    re_tracing::profile_function!();
    use xshell::Shell;

    let sh = Shell::new().unwrap();
    let tmp = sh.create_temp_dir().unwrap();
    let tmp_path = Utf8PathBuf::try_from(tmp.path().to_path_buf()).unwrap();

    let entrypoint_path = entrypoint_path.as_ref();
    let entrypoint_filename = entrypoint_path.file_name().unwrap();

    let include_dir_path = include_dir_path.as_ref();
    let include_dir_path = include_dir_path
        .canonicalize_utf8()
        .with_context(|| format!("failed to canonicalize include path: {include_dir_path:?}"))
        .unwrap();

    // generate bfbs definitions
    compile_binary_schemas(&include_dir_path, &tmp_path, entrypoint_path);

    let mut binary_entrypoint_path = Utf8PathBuf::from(entrypoint_filename);
    binary_entrypoint_path.set_extension("bfbs");

    // semantic pass: high level objects from low-level reflection data
    let mut objects = Objects::from_buf(
        include_dir_path,
        sh.read_binary_file(tmp_path.join(binary_entrypoint_path))
            .unwrap()
            .as_slice(),
    );

    // create and fill out arrow registry
    let mut arrow_registry = ArrowRegistry::default();
    for obj in objects.ordered_objects_mut(None) {
        arrow_registry.register(obj);
    }

    (objects, arrow_registry)
}

/// Generates .gitattributes files that mark up all generated files as generated.
fn generate_gitattributes_for_generated_files(files_to_write: &mut GeneratedFiles) {
    re_tracing::profile_function!();

    const FILENAME: &str = ".gitattributes";

    let mut filepaths_per_folder = BTreeMap::default();

    for filepath in files_to_write.keys() {
        let dirpath = filepath.parent().unwrap();
        let files: &mut Vec<_> = filepaths_per_folder.entry(dirpath.to_owned()).or_default();
        files.push(filepath.clone());
    }

    for (dirpath, files) in filepaths_per_folder {
        let gitattributes_path = dirpath.join(FILENAME);

        let generated_files = std::iter::once(FILENAME.to_owned()) // The attributes itself is generated!
            .chain(files.iter().map(|filepath| {
                format_path(
                    filepath
                        .strip_prefix(&dirpath)
                        .context("Failed to make {filepath} relative to {dirpath}.")
                        .unwrap(),
                )
            }))
            .map(|s| format!("{s} linguist-generated=true"))
            .collect::<Vec<_>>();

        let content = format!(
            "# DO NOT EDIT! This file is generated by {}\n\n{}\n",
            format_path(file!()),
            generated_files.join("\n")
        );

        files_to_write.insert(gitattributes_path, content);
    }
}

/// This will automatically emit a `rerun-if-changed` clause for all the files that were hashed.
pub fn compute_re_types_builder_hash() -> String {
    compute_crate_hash("re_types_builder")
}

pub struct SourceLocations<'a> {
    pub definitions_dir: &'a str,
    pub doc_examples_dir: &'a str,
    pub python_output_dir: &'a str,
    pub cpp_output_dir: &'a str,
}

/// Also triggers a re-build if anything that affects the hash changes.
pub fn compute_re_types_hash(locations: &SourceLocations<'_>) -> String {
    // NOTE: We need to hash both the flatbuffers definitions as well as the source code of the
    // code generator itself!
    let re_types_builder_hash = compute_re_types_builder_hash();
    let definitions_hash = compute_dir_hash(locations.definitions_dir, Some(&["fbs"]));
    let doc_examples_hash =
        compute_dir_hash(locations.doc_examples_dir, Some(&["rs", "py", "cpp"]));
    let python_extensions_hash = compute_dir_filtered_hash(locations.python_output_dir, |path| {
        path.to_str().unwrap().ends_with("_ext.py")
    });
    let cpp_extensions_hash = compute_dir_filtered_hash(locations.cpp_output_dir, |path| {
        path.to_str().unwrap().ends_with("_ext.cpp")
    });

    let new_hash = compute_strings_hash(&[
        &re_types_builder_hash,
        &definitions_hash,
        &doc_examples_hash,
        &python_extensions_hash,
        &cpp_extensions_hash,
    ]);

    re_log::debug!("re_types_builder_hash: {re_types_builder_hash:?}");
    re_log::debug!("definitions_hash: {definitions_hash:?}");
    re_log::debug!("doc_examples_hash: {doc_examples_hash:?}");
    re_log::debug!("python_extensions_hash: {python_extensions_hash:?}");
    re_log::debug!("cpp_extensions_hash: {cpp_extensions_hash:?}");
    re_log::debug!("new_hash: {new_hash:?}");

    new_hash
}

/// Generates C++ code.
///
/// Panics on error.
///
/// - `output_path`: path to the root of the output.
///
/// E.g.:
/// ```no_run
/// let (objects, arrow_registry) = re_types_builder::generate_lang_agnostic(
///     "./definitions",
///     "./definitions/rerun/archetypes.fbs",
/// );
/// # let reporter = re_types_builder::report::init().1;
/// re_types_builder::generate_cpp_code(
///     &reporter,
///     ".",
///     &objects,
///     &arrow_registry,
/// );
/// ```
pub fn generate_cpp_code(
    reporter: &Reporter,
    output_path: impl AsRef<Utf8Path>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();

    // 1. Generate code files.
    let mut gen = CppCodeGenerator::new(output_path.as_ref());
    let mut files = gen.generate(reporter, objects, arrow_registry);
    // 2. Generate attribute files.
    generate_gitattributes_for_generated_files(&mut files);
    // 3. Write all files.
    {
        use rayon::prelude::*;

        re_tracing::profile_scope!("write_files");

        files.par_iter().for_each(|(filepath, contents)| {
            // There's more than cpp/hpp files in here, don't run clang-format on them!
            let contents = if matches!(filepath.extension(), Some("cpp" | "hpp")) {
                format_code(contents)
            } else {
                contents.clone()
            };
            crate::codegen::common::write_file(filepath, &contents);
        });
    }
    // 4. Remove orphaned files.
    // NOTE: In rerun_cpp we have a directory where we share generated code with handwritten code.
    // Make sure to filter out that directory, or else we will end up removing those handwritten
    // files.
    let root_src = output_path.as_ref().join("src/rerun");
    files.retain(|filepath, _| filepath.parent() != Some(root_src.as_path()));
    crate::codegen::common::remove_orphaned_files(reporter, &files);

    fn format_code(code: &str) -> String {
        clang_format::clang_format_with_style(code, &clang_format::ClangFormatStyle::File)
            .expect("Failed to run clang-format")
    }
}

/// Generates Rust code.
///
/// Panics on error.
///
/// - `output_crate_path`: path to the root of the output crate.
///
/// E.g.:
/// ```no_run
/// let (objects, arrow_registry) = re_types_builder::generate_lang_agnostic(
///     "./definitions",
///     "./definitions/rerun/archetypes.fbs",
/// );
/// # let reporter = re_types_builder::report::init().1;
/// re_types_builder::generate_rust_code(
///     &reporter,
///     ".",
///     &objects,
///     &arrow_registry,
/// );
/// ```
pub fn generate_rust_code(
    reporter: &Reporter,
    workspace_path: impl Into<Utf8PathBuf>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();

    // 1. Generate code files.
    let mut gen = RustCodeGenerator::new(workspace_path);
    let mut files = gen.generate(reporter, objects, arrow_registry);
    // 2. Generate attribute files.
    generate_gitattributes_for_generated_files(&mut files);
    // 3. Write all files.
    write_files(&files);
    // 4. Remove orphaned files.
    crate::codegen::common::remove_orphaned_files(reporter, &files);

    fn write_files(files: &GeneratedFiles) {
        use rayon::prelude::*;

        re_tracing::profile_function!();

        files.par_iter().for_each(|(path, source)| {
            write_file(path, source.clone());
        });
    }

    fn write_file(filepath: &Utf8PathBuf, mut contents: String) {
        re_tracing::profile_function!();

        contents = contents.replace(" :: ", "::"); // Fix `bytemuck :: Pod` -> `bytemuck::Pod`.

        // Even though we already have used `prettyplease` we also
        // need to run `cargo fmt`, since it catches some things `prettyplease` missed.
        // We need to run `cago fmt` several times because it is not idempotent;
        // see https://github.com/rust-lang/rustfmt/issues/5824
        for _ in 0..2 {
            // NOTE: We're purposefully ignoring the error here.
            //
            // In the very unlikely chance that the user doesn't have the `fmt` component installed,
            // there's still no good reason to fail the build.
            //
            // The CI will catch the unformatted file at PR time and complain appropriately anyhow.

            re_tracing::profile_scope!("rust-fmt");
            use rust_format::Formatter as _;
            if let Ok(formatted) = rust_format::RustFmt::default().format_str(&contents) {
                contents = formatted;
            }
        }

        crate::codegen::common::write_file(filepath, &contents);
    }
}

/// Generates Python code.
///
/// Panics on error.
///
/// - `output_pkg_path`: path to the root of the output package.
///
/// E.g.:
/// ```no_run
/// let (objects, arrow_registry) = re_types_builder::generate_lang_agnostic(
///     "./definitions",
///     "./definitions/rerun/archetypes.fbs",
/// );
/// # let reporter = re_types_builder::report::init().1;
/// re_types_builder::generate_python_code(
///     &reporter,
///     "./rerun_py/rerun_sdk",
///     "./rerun_py/tests",
///     &objects,
///     &arrow_registry,
/// );
/// ```
pub fn generate_python_code(
    reporter: &Reporter,
    output_pkg_path: impl AsRef<Utf8Path>,
    testing_output_pkg_path: impl AsRef<Utf8Path>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();

    // 1. Generate code files.
    let mut gen =
        PythonCodeGenerator::new(output_pkg_path.as_ref(), testing_output_pkg_path.as_ref());
    let mut files = gen.generate(reporter, objects, arrow_registry);
    // 2. Generate attribute files.
    generate_gitattributes_for_generated_files(&mut files);
    // 3. Write all files.
    write_files(&gen.pkg_path, &gen.testing_pkg_path, &files);
    // 4. Remove orphaned files.
    crate::codegen::common::remove_orphaned_files(reporter, &files);

    fn write_files(pkg_path: &Utf8Path, testing_pkg_path: &Utf8Path, files: &GeneratedFiles) {
        use rayon::prelude::*;

        re_tracing::profile_function!();

        // Running `black` once for each file is very slow, so we write all
        // files to a temporary folder, format it, and copy back the results.

        let tempdir = tempfile::tempdir().unwrap();
        let tempdir_path = Utf8PathBuf::try_from(tempdir.path().to_owned()).unwrap();

        files.par_iter().for_each(|(filepath, source)| {
            let formatted_source_path =
                format_path_for_tmp_dir(pkg_path, testing_pkg_path, filepath, &tempdir_path);
            crate::codegen::common::write_file(&formatted_source_path, source);
        });

        format_python_dir(&tempdir_path).unwrap();

        // Read back and copy to the final destination:
        files.par_iter().for_each(|(filepath, _original_source)| {
            let formatted_source_path =
                format_path_for_tmp_dir(pkg_path, testing_pkg_path, filepath, &tempdir_path);
            let formatted_source = std::fs::read_to_string(formatted_source_path).unwrap();
            crate::codegen::common::write_file(filepath, &formatted_source);
        });
    }

    fn format_path_for_tmp_dir(
        pkg_path: &Utf8Path,
        testing_pkg_path: &Utf8Path,
        filepath: &Utf8Path,
        tempdir_path: &Utf8Path,
    ) -> Utf8PathBuf {
        // If the prefix is pkg_path, strip it, and then append to tempdir
        // However, if the prefix is testing_pkg_path, strip it and insert an extra
        // "testing" to avoid name collisions.
        filepath.strip_prefix(pkg_path).map_or_else(
            |_| {
                tempdir_path
                    .join("testing")
                    .join(filepath.strip_prefix(testing_pkg_path).unwrap())
            },
            |f| tempdir_path.join(f),
        )
    }

    fn format_python_dir(dir: &Utf8PathBuf) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        // The order below is important and sadly we need to call black twice. Ruff does not yet
        // fix line-length (See: https://github.com/astral-sh/ruff/issues/1904).
        //
        // 1) Call black, which among others things fixes line-length
        // 2) Call ruff, which requires line-lengths to be correct
        // 3) Call black again to cleanup some whitespace issues ruff might introduce

        run_black_on_dir(dir).context("black")?;
        run_ruff_on_dir(dir).context("ruff")?;
        run_black_on_dir(dir).context("black")?;
        Ok(())
    }

    fn python_project_path() -> Utf8PathBuf {
        let path = crate::rerun_workspace_path()
            .join("rerun_py")
            .join("pyproject.toml");
        assert!(path.exists(), "Failed to find {path:?}");
        path
    }

    fn run_black_on_dir(dir: &Utf8PathBuf) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        use std::process::{Command, Stdio};

        let proc = Command::new("black")
            .arg(format!("--config={}", python_project_path()))
            .arg(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let output = proc.wait_with_output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            anyhow::bail!("{stdout}\n{stderr}")
        }
    }

    fn run_ruff_on_dir(dir: &Utf8PathBuf) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        use std::process::{Command, Stdio};

        let proc = Command::new("ruff")
            .arg(format!("--config={}", python_project_path()))
            .arg("--fix")
            .arg(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let output = proc.wait_with_output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            anyhow::bail!("{stdout}\n{stderr}")
        }
    }
}

pub fn generate_docs(
    reporter: &Reporter,
    output_docs_dir: impl AsRef<Utf8Path>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();

    re_log::info!("Generating docs to {}", output_docs_dir.as_ref());

    // 1. Generate code files.
    let mut gen = DocsCodeGenerator::new(output_docs_dir.as_ref());
    let mut files = gen.generate(reporter, objects, arrow_registry);
    // 2. Generate attribute files.
    generate_gitattributes_for_generated_files(&mut files);
    // 3. Write all files.
    {
        use rayon::prelude::*;

        re_tracing::profile_scope!("write_files");

        files.par_iter().for_each(|(filepath, contents)| {
            crate::codegen::common::write_file(filepath, contents);
        });
    }
    // 4. Remove orphaned files.
    crate::codegen::common::remove_orphaned_files(reporter, &files);
}

pub(crate) fn rerun_workspace_path() -> camino::Utf8PathBuf {
    let workspace_root = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_dir = camino::Utf8PathBuf::from(manifest_dir);
        manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    } else {
        let file_path = camino::Utf8PathBuf::from(file!());
        file_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    };

    assert!(
        workspace_root.exists(),
        "Failed to find workspace root, expected it at {workspace_root:?}"
    );

    // Check for something that only exists in root:
    assert!(
        workspace_root.join("CODE_OF_CONDUCT.md").exists(),
        "Failed to find workspace root, expected it at {workspace_root:?}"
    );

    workspace_root.canonicalize_utf8().unwrap()
}

// ---

/// Converts a snake or pascal case input into a snake case output.
///
/// If the input contains multiple parts separated by dots, only the last part is converted.
pub(crate) fn to_snake_case(s: &str) -> String {
    use convert_case::{Boundary, Converter, Pattern};

    let rerun_snake = Converter::new()
        .set_boundaries(&[
            Boundary::Hyphen,
            Boundary::Space,
            Boundary::Underscore,
            Boundary::Acronym,
            Boundary::LowerUpper,
        ])
        .set_pattern(Pattern::Lowercase)
        .set_delim("_");

    let mut parts: Vec<_> = s.split('.').map(ToOwned::to_owned).collect();
    if let Some(last) = parts.last_mut() {
        *last = last.replace("UVec", "uvec").replace("UInt", "uint");
        *last = rerun_snake.convert(&last);
    }
    parts.join(".")
}

#[test]
fn test_to_snake_case() {
    assert_eq!(
        to_snake_case("rerun.components.Position2D"),
        "rerun.components.position2d"
    );
    assert_eq!(
        to_snake_case("rerun.components.position2d"),
        "rerun.components.position2d"
    );

    assert_eq!(
        to_snake_case("rerun.datatypes.Utf8"),
        "rerun.datatypes.utf8"
    );
    assert_eq!(
        to_snake_case("rerun.datatypes.utf8"),
        "rerun.datatypes.utf8"
    );

    assert_eq!(
        to_snake_case("rerun.datatypes.UVec2D"),
        "rerun.datatypes.uvec2d"
    );
    assert_eq!(
        to_snake_case("rerun.datatypes.uvec2d"),
        "rerun.datatypes.uvec2d"
    );

    assert_eq!(
        to_snake_case("rerun.datatypes.UInt32"),
        "rerun.datatypes.uint32"
    );
    assert_eq!(
        to_snake_case("rerun.datatypes.uint32"),
        "rerun.datatypes.uint32"
    );

    assert_eq!(
        to_snake_case("rerun.archetypes.Points2DIndicator"),
        "rerun.archetypes.points2d_indicator"
    );
    assert_eq!(
        to_snake_case("rerun.archetypes.points2d_indicator"),
        "rerun.archetypes.points2d_indicator"
    );

    assert_eq!(
        to_snake_case("rerun.components.TranslationAndMat3x3"),
        "rerun.components.translation_and_mat3x3"
    );
    assert_eq!(
        to_snake_case("rerun.components.translation_and_mat3x3"),
        "rerun.components.translation_and_mat3x3"
    );

    assert_eq!(
        to_snake_case("rerun.components.AnnotationContext"),
        "rerun.components.annotation_context"
    );
}

/// Converts a snake or pascal case input into a pascal case output.
///
/// If the input contains multiple parts separated by dots, only the last part is converted.
pub(crate) fn to_pascal_case(s: &str) -> String {
    use convert_case::{Boundary, Converter, Pattern};

    let rerun_snake = Converter::new()
        .set_boundaries(&[
            Boundary::Hyphen,
            Boundary::Space,
            Boundary::Underscore,
            Boundary::DigitUpper,
            Boundary::Acronym,
            Boundary::LowerUpper,
        ])
        .set_pattern(Pattern::Capital);

    let mut parts: Vec<_> = s.split('.').map(ToOwned::to_owned).collect();
    if let Some(last) = parts.last_mut() {
        *last = last
            .replace("uvec", "UVec")
            .replace("uint", "UInt")
            .replace("2d", "2D")
            .replace("3d", "3D")
            .replace("4d", "4D");
        *last = rerun_snake.convert(&last);
    }
    parts.join(".")
}

/// Format the path with forward slashes, even on Windows.
pub(crate) fn format_path(path: impl AsRef<Utf8Path>) -> String {
    path.as_ref().as_str().replace('\\', "/")
}

#[test]
fn test_to_pascal_case() {
    assert_eq!(
        to_pascal_case("rerun.components.position2d"),
        "rerun.components.Position2D"
    );
    assert_eq!(
        to_pascal_case("rerun.components.Position2D"),
        "rerun.components.Position2D"
    );

    assert_eq!(
        to_pascal_case("rerun.datatypes.uvec2d"),
        "rerun.datatypes.UVec2D"
    );
    assert_eq!(
        to_pascal_case("rerun.datatypes.UVec2D"),
        "rerun.datatypes.UVec2D"
    );

    assert_eq!(
        to_pascal_case("rerun.datatypes.uint32"),
        "rerun.datatypes.UInt32"
    );
    assert_eq!(
        to_pascal_case("rerun.datatypes.UInt32"),
        "rerun.datatypes.UInt32"
    );

    assert_eq!(
        to_pascal_case("rerun.archetypes.points2d_indicator"),
        "rerun.archetypes.Points2DIndicator"
    );
    assert_eq!(
        to_pascal_case("rerun.archetypes.Points2DIndicator"),
        "rerun.archetypes.Points2DIndicator"
    );

    assert_eq!(
        to_pascal_case("rerun.components.translation_and_mat3x3"),
        "rerun.components.TranslationAndMat3x3"
    );
    assert_eq!(
        to_pascal_case("rerun.components.TranslationAndMat3x3"),
        "rerun.components.TranslationAndMat3x3"
    );
}
