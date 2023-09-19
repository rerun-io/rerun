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

use anyhow::Context;

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

pub use self::arrow_registry::{ArrowRegistry, LazyDatatype, LazyField};
pub use self::codegen::{CodeGenerator, CppCodeGenerator, PythonCodeGenerator, RustCodeGenerator};
pub use self::objects::{
    Attributes, Docs, ElementType, Object, ObjectField, ObjectKind, ObjectSpecifics, Objects, Type,
};

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
pub const ATTR_RUST_REPR: &str = "attr.rust.repr";
pub const ATTR_RUST_TUPLE_STRUCT: &str = "attr.rust.tuple_struct";
pub const ATTR_RUST_NEW_PUB_CRATE: &str = "attr.rust.new_pub_crate";

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

/// Generates a .gitattributes file that marks up all generated files as generated
pub fn generate_gitattributes_for_generated_files(
    output_path: &impl AsRef<Utf8Path>,
    files: impl Iterator<Item = Utf8PathBuf>,
) {
    let filename = ".gitattributes";
    let path = output_path.as_ref().join(filename);

    let generated_files = std::iter::once(filename.to_owned()) // The attributes itself is generated!
        .chain(files.map(|path| {
            path.strip_prefix(output_path.as_ref().as_std_path())
                .context("Failed to make path relative to output path.")
                .unwrap()
                .to_string()
        }))
        .map(|s| format!("{s} linguist-generated=true\n"))
        .collect::<Vec<_>>();

    let content = format!(
        "# DO NOT EDIT! This file is generated by {}\n\n{}",
        file!(),
        generated_files.join("")
    );

    codegen::write_file(&path, &content);
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
/// re_types_builder::generate_cpp_code(
///     ".",
///     &objects,
///     &arrow_registry,
/// );
/// ```
pub fn generate_cpp_code(
    output_path: impl AsRef<Utf8Path>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();
    let mut gen = CppCodeGenerator::new(output_path.as_ref());
    let filepaths = gen.generate(objects, arrow_registry);
    generate_gitattributes_for_generated_files(&output_path, filepaths.into_iter());
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
/// re_types_builder::generate_rust_code(
///     ".",
///     &objects,
///     &arrow_registry,
/// );
/// ```
pub fn generate_rust_code(
    output_crate_path: impl AsRef<Utf8Path>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();
    let mut gen = RustCodeGenerator::new(output_crate_path.as_ref());
    let filepaths = gen.generate(objects, arrow_registry);
    generate_gitattributes_for_generated_files(&output_crate_path, filepaths.into_iter());
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
/// re_types_builder::generate_python_code(
///     "./rerun_py",
///     &objects,
///     &arrow_registry,
/// );
/// ```
pub fn generate_python_code(
    output_pkg_path: impl AsRef<Utf8Path>,
    testing_output_pkg_path: impl AsRef<Utf8Path>,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
) {
    re_tracing::profile_function!();
    let mut gen =
        PythonCodeGenerator::new(output_pkg_path.as_ref(), testing_output_pkg_path.as_ref());
    let filepaths = gen.generate(objects, arrow_registry);
    generate_gitattributes_for_generated_files(
        &output_pkg_path,
        filepaths
            .iter()
            .filter(|f| f.starts_with(output_pkg_path.as_ref()))
            .cloned(),
    );
    generate_gitattributes_for_generated_files(
        &testing_output_pkg_path,
        filepaths
            .into_iter()
            .filter(|f| f.starts_with(testing_output_pkg_path.as_ref())),
    );
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

    workspace_root
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
            .replace("2d", "2D")
            .replace("3d", "3D")
            .replace("4d", "4D");
        *last = rerun_snake.convert(&last);
    }
    parts.join(".")
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
