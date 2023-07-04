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
pub use self::codegen::{CodeGenerator, PythonCodeGenerator, RustCodeGenerator};
pub use self::objects::{
    Attributes, Docs, ElementType, Object, ObjectField, ObjectKind, Objects, Type,
};

// --- Attributes ---

pub const ATTR_ARROW_TRANSPARENT: &str = "attr.arrow.transparent";
pub const ATTR_ARROW_SPARSE_UNION: &str = "attr.arrow.sparse_union";

pub const ATTR_RERUN_COMPONENT_OPTIONAL: &str = "attr.rerun.component_optional";
pub const ATTR_RERUN_COMPONENT_RECOMMENDED: &str = "attr.rerun.component_recommended";
pub const ATTR_RERUN_COMPONENT_REQUIRED: &str = "attr.rerun.component_required";
pub const ATTR_RERUN_LEGACY_FQNAME: &str = "attr.rerun.legacy_fqname";

pub const ATTR_PYTHON_TRANSPARENT: &str = "attr.python.transparent";
pub const ATTR_PYTHON_ALIASES: &str = "attr.python.aliases";
pub const ATTR_PYTHON_ARRAY_ALIASES: &str = "attr.python.array_aliases";

pub const ATTR_RUST_DERIVE: &str = "attr.rust.derive";
pub const ATTR_RUST_REPR: &str = "attr.rust.repr";
pub const ATTR_RUST_TUPLE_STRUCT: &str = "attr.rust.tuple_struct";

// --- Entrypoints ---

use std::path::{Path, PathBuf};

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
    include_dir_path: impl AsRef<Path>,
    output_dir_path: impl AsRef<Path>,
    entrypoint_path: impl AsRef<Path>,
) {
    let include_dir_path = include_dir_path.as_ref().to_str().unwrap();
    let output_dir_path = output_dir_path.as_ref().to_str().unwrap();
    let entrypoint_path = entrypoint_path.as_ref().to_str().unwrap();

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
fn generate_lang_agnostic(
    include_dir_path: impl AsRef<Path>,
    entrypoint_path: impl AsRef<Path>,
) -> (Objects, ArrowRegistry) {
    use xshell::Shell;

    let sh = Shell::new().unwrap();
    let tmp = sh.create_temp_dir().unwrap();

    let entrypoint_path = entrypoint_path.as_ref();
    let entrypoint_filename = entrypoint_path.file_name().unwrap();

    // generate bfbs definitions
    compile_binary_schemas(include_dir_path, tmp.path(), entrypoint_path);

    let mut binary_entrypoint_path = PathBuf::from(entrypoint_filename);
    binary_entrypoint_path.set_extension("bfbs");

    // semantic pass: high level objects from low-level reflection data
    let mut objects = Objects::from_buf(
        sh.read_binary_file(tmp.path().join(binary_entrypoint_path))
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

/// Generates Rust code from a set of flatbuffers definitions.
///
/// Panics on error.
///
/// - `include_dir_path`: path to the root directory of the fbs definition tree.
/// - `output_crate_path`: path to the root of the output crate.
/// - `entrypoint_path`: path to the root file of the fbs definition tree.
///
/// E.g.:
/// ```no_run
/// re_types_builder::generate_rust_code(
///     "./definitions",
///     ".",
///     "./definitions/rerun/archetypes.fbs",
/// );
/// ```
pub fn generate_rust_code(
    include_dir_path: impl AsRef<Path>,
    output_crate_path: impl AsRef<Path>,
    entrypoint_path: impl AsRef<Path>,
) {
    // passes 1 through 3: bfbs, semantic, arrow registry
    let (objects, arrow_registry) = generate_lang_agnostic(include_dir_path, entrypoint_path);

    let mut gen = RustCodeGenerator::new(output_crate_path.as_ref());
    let _filepaths = gen.generate(&objects, &arrow_registry);
}

/// Generates Python code from a set of flatbuffers definitions.
///
/// Panics on error.
///
/// - `include_dir_path`: path to the root directory of the fbs definition tree.
/// - `output_pkg_path`: path to the root of the output package.
/// - `entrypoint_path`: path to the root file of the fbs definition tree.
///
/// E.g.:
/// ```no_run
/// re_types_builder::generate_python_code(
///     "./definitions",
///     "./rerun_py",
///     "./definitions/rerun/archetypes.fbs",
/// );
/// ```
pub fn generate_python_code(
    include_dir_path: impl AsRef<Path>,
    output_pkg_path: impl AsRef<Path>,
    entrypoint_path: impl AsRef<Path>,
) {
    // passes 1 through 3: bfbs, semantic, arrow registry
    let (objects, arrow_registry) = generate_lang_agnostic(include_dir_path, entrypoint_path);

    // generate python code
    let mut gen = PythonCodeGenerator::new(output_pkg_path.as_ref());
    let _filepaths = gen.generate(&objects, &arrow_registry);
}
