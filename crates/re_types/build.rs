//! Generates Rust & Python code from flatbuffers definitions.

use std::path::PathBuf;

use re_build_tools::{
    compute_crate_hash, compute_dir_filtered_hash, compute_dir_hash, compute_strings_hash,
    is_tracked_env_var_set, iter_dir, read_versioning_hash, rerun_if_changed,
    rerun_if_changed_or_doesnt_exist, write_versioning_hash,
};

// ---

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const DEFINITIONS_DIR_PATH: &str = "./definitions";
const ENTRYPOINT_PATH: &str = "./definitions/rerun/archetypes.fbs";
const DOC_EXAMPLES_DIR_PATH: &str = "../../docs/code-examples";
const CPP_OUTPUT_DIR_PATH: &str = "../../rerun_cpp";
const RUST_OUTPUT_DIR_PATH: &str = ".";
const PYTHON_OUTPUT_DIR_PATH: &str = "../../rerun_py/rerun_sdk/rerun/_rerun2";

// located in PYTHON_OUTPUT_DIR_PATH
const ARCHETYPE_OVERRIDES_SUB_DIR_PATH: &str = "archetypes/_overrides";
const COMPONENT_OVERRIDES_SUB_DIR_PATH: &str = "components/_overrides";
const DATATYPE_OVERRIDES_SUB_DIR_PATH: &str = "datatypes/_overrides";

fn main() {
    if cfg!(target_os = "windows") {
        // TODO(#2591): Codegen is temporarily disabled on Windows due to hashing issues.
        return;
    }

    if !is_tracked_env_var_set("IS_IN_RERUN_WORKSPACE") {
        // Only run if we are in the rerun workspace, not on users machines.
        return;
    }
    if is_tracked_env_var_set("RERUN_IS_PUBLISHING") {
        // We don't need to rebuild - we should have done so beforehand!
        // See `RELEASES.md`
        return;
    }

    rerun_if_changed_or_doesnt_exist(SOURCE_HASH_PATH);
    for path in iter_dir(DEFINITIONS_DIR_PATH, Some(&["fbs"])) {
        rerun_if_changed(&path);
    }

    // NOTE: We need to hash both the flatbuffers definitions as well as the source code of the
    // code generator itself!
    let cur_hash = read_versioning_hash(SOURCE_HASH_PATH);
    let re_types_builder_hash = compute_crate_hash("re_types_builder");
    let definitions_hash = compute_dir_hash(DEFINITIONS_DIR_PATH, Some(&["fbs"]));
    let doc_examples_hash = compute_dir_hash(DOC_EXAMPLES_DIR_PATH, Some(&["rs", "py", "cpp"]));
    let python_archetype_overrides_hash = compute_dir_hash(
        PathBuf::from(PYTHON_OUTPUT_DIR_PATH).join(ARCHETYPE_OVERRIDES_SUB_DIR_PATH),
        Some(&["py"]),
    );
    let python_component_overrides_hash = compute_dir_hash(
        PathBuf::from(PYTHON_OUTPUT_DIR_PATH).join(COMPONENT_OVERRIDES_SUB_DIR_PATH),
        Some(&["py"]),
    );
    let python_datatype_overrides_hash = compute_dir_hash(
        PathBuf::from(PYTHON_OUTPUT_DIR_PATH).join(DATATYPE_OVERRIDES_SUB_DIR_PATH),
        Some(&["py"]),
    );
    let cpp_extensions_hash = compute_dir_filtered_hash(CPP_OUTPUT_DIR_PATH, |path| {
        path.to_str().unwrap().ends_with("_ext.cpp")
    });

    let new_hash = compute_strings_hash(&[
        &re_types_builder_hash,
        &definitions_hash,
        &doc_examples_hash,
        &python_archetype_overrides_hash,
        &python_component_overrides_hash,
        &python_datatype_overrides_hash,
        &cpp_extensions_hash,
    ]);

    // Leave these be please, very useful when debugging.
    eprintln!("re_types_builder_hash: {re_types_builder_hash:?}");
    eprintln!("definitions_hash: {definitions_hash:?}");
    eprintln!("doc_examples_hash: {doc_examples_hash:?}");
    eprintln!("python_archetype_overrides_hash: {python_archetype_overrides_hash:?}");
    eprintln!("python_component_overrides_hash: {python_component_overrides_hash:?}");
    eprintln!("python_datatype_overrides_hash: {python_datatype_overrides_hash:?}");
    eprintln!("cpp_extensions_hash: {cpp_extensions_hash:?}");
    eprintln!("new_hash: {new_hash:?}");
    eprintln!("cur_hash: {cur_hash:?}");

    if let Some(cur_hash) = cur_hash {
        if cur_hash == new_hash {
            // Neither the source of the code generator nor the IDL definitions have changed, no need
            // to do anything at this point.
            return;
        }
    }

    // Detect desyncs between definitions and generated when running on CI, and
    // crash the build accordingly.
    #[allow(clippy::manual_assert)]
    if std::env::var("CI").is_ok() {
        panic!("re_types' fbs definitions and generated code are out-of-sync!");
    }

    // passes 1 through 3: bfbs, semantic, arrow registry
    let (objects, arrow_registry) =
        re_types_builder::generate_lang_agnostic(DEFINITIONS_DIR_PATH, ENTRYPOINT_PATH);

    join3(
        || re_types_builder::generate_cpp_code(CPP_OUTPUT_DIR_PATH, &objects, &arrow_registry),
        || re_types_builder::generate_rust_code(RUST_OUTPUT_DIR_PATH, &objects, &arrow_registry),
        || {
            re_types_builder::generate_python_code(
                PYTHON_OUTPUT_DIR_PATH,
                &objects,
                &arrow_registry,
            );
        },
    );

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}

// Do 3 things in parallel
fn join3(a: impl FnOnce() + Send, b: impl FnOnce() + Send, c: impl FnOnce() + Send) {
    rayon::join(a, || rayon::join(b, c));
}
