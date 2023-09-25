//! Generates Rust & Python code from flatbuffers definitions.

use std::path::Path;

use re_build_tools::{
    is_tracked_env_var_set, iter_dir, read_versioning_hash, rerun_if_changed, write_versioning_hash,
};
use re_types_builder::{compute_re_types_hash, SourceLocations};

// ---

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const DEFINITIONS_DIR_PATH: &str = "./definitions";
const ENTRYPOINT_PATH: &str = "./definitions/rerun/archetypes.fbs";
const DOC_EXAMPLES_DIR_PATH: &str = "../../docs/code-examples";
const CPP_OUTPUT_DIR_PATH: &str = "../../rerun_cpp";
const RUST_OUTPUT_DIR_PATH: &str = ".";
const PYTHON_OUTPUT_DIR_PATH: &str = "../../rerun_py/rerun_sdk/rerun";
const PYTHON_TESTING_OUTPUT_DIR_PATH: &str = "../../rerun_py/tests/test_types";

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

    // Only re-build if source-hash exists
    if !Path::new(SOURCE_HASH_PATH).exists() {
        return;
    }

    rerun_if_changed(SOURCE_HASH_PATH);
    for path in iter_dir(DEFINITIONS_DIR_PATH, Some(&["fbs"])) {
        rerun_if_changed(&path);
    }

    // NOTE: We need to hash both the flatbuffers definitions as well as the source code of the
    // code generator itself!
    let cur_hash = read_versioning_hash(SOURCE_HASH_PATH);
    eprintln!("cur_hash: {cur_hash:?}");

    let new_hash = compute_re_types_hash(&SourceLocations {
        definitions_dir: DEFINITIONS_DIR_PATH,
        doc_examples_dir: DOC_EXAMPLES_DIR_PATH,
        python_output_dir: PYTHON_OUTPUT_DIR_PATH,
        cpp_output_dir: CPP_OUTPUT_DIR_PATH,
    });

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

    let (report, reporter) = re_types_builder::report::init();

    // passes 1 through 3: bfbs, semantic, arrow registry
    let (objects, arrow_registry) =
        re_types_builder::generate_lang_agnostic(DEFINITIONS_DIR_PATH, ENTRYPOINT_PATH);

    join3(
        || {
            re_types_builder::generate_cpp_code(
                &reporter,
                CPP_OUTPUT_DIR_PATH,
                &objects,
                &arrow_registry,
            );
        },
        || {
            re_types_builder::generate_rust_code(
                &reporter,
                RUST_OUTPUT_DIR_PATH,
                &objects,
                &arrow_registry,
            );
        },
        || {
            re_types_builder::generate_python_code(
                &reporter,
                PYTHON_OUTPUT_DIR_PATH,
                PYTHON_TESTING_OUTPUT_DIR_PATH,
                &objects,
                &arrow_registry,
            );
        },
    );

    report.panic_on_errors();

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}

// Do 3 things in parallel
fn join3(a: impl FnOnce() + Send, b: impl FnOnce() + Send, c: impl FnOnce() + Send) {
    rayon::join(a, || rayon::join(b, c));
}
