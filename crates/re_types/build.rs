//! Generates Rust & Python code from flatbuffers definitions.

use std::path::Path;

use re_build_tools::{iter_dir, read_versioning_hash, rerun_if_changed, write_versioning_hash};
use re_types_builder::{compute_re_types_hash, SourceLocations};

// ---

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const DEFINITIONS_DIR_PATH: &str = "./definitions";
const ENTRYPOINT_PATH: &str = "./definitions/rerun/archetypes.fbs";
const DOC_EXAMPLES_DIR_PATH: &str = "../../docs/code-examples";
const DOC_CONTENT_DIR_PATH: &str = "../../docs/content/reference/types";
const CPP_OUTPUT_DIR_PATH: &str = "../../rerun_cpp";
const RUST_OUTPUT_DIR_PATH: &str = ".";
const PYTHON_OUTPUT_DIR_PATH: &str = "../../rerun_py/rerun_sdk/rerun";
const PYTHON_TESTING_OUTPUT_DIR_PATH: &str = "../../rerun_py/tests/test_types";

/// This uses [`rayon::scope`] to spawn all closures as tasks
/// running in parallel. It blocks until all tasks are done.
macro_rules! join {
    ($($task:expr,)*) => {join!($($task),*)};
    ($($task:expr),*) => {{
        #![allow(clippy::redundant_closure_call)]
        ::rayon::scope(|scope| {
            $(scope.spawn(|_| ($task)());)*
        })
    }}
}

fn should_run() -> bool {
    #![allow(clippy::match_same_arms)]
    use re_build_tools::Environment;

    if cfg!(target_os = "windows") {
        // TODO(#2591): Codegen is currently disabled on Windows due to hashing issues, likely because of `\r` in files
        return false;
    }

    match Environment::detect() {
        // we should have been run before publishing
        Environment::PublishingCrates => false,

        // YES! We run it to verify that the generated code is up-to-date.
        Environment::CI => true,

        Environment::DeveloperInWorkspace => true,

        // We ship pre-built source files for users
        Environment::ProbablyUserMachine => false,
    }
}

fn main() {
    if !should_run() {
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
    if re_build_tools::is_on_ci() {
        panic!("re_types' fbs definitions and generated code are out-of-sync!");
    }

    let (report, reporter) = re_types_builder::report::init();

    // passes 1 through 3: bfbs, semantic, arrow registry
    let (objects, arrow_registry) =
        re_types_builder::generate_lang_agnostic(DEFINITIONS_DIR_PATH, ENTRYPOINT_PATH);

    join!(
        || re_types_builder::generate_cpp_code(
            &reporter,
            CPP_OUTPUT_DIR_PATH,
            &objects,
            &arrow_registry,
        ),
        || re_types_builder::generate_rust_code(
            &reporter,
            RUST_OUTPUT_DIR_PATH,
            &objects,
            &arrow_registry,
        ),
        || re_types_builder::generate_python_code(
            &reporter,
            PYTHON_OUTPUT_DIR_PATH,
            PYTHON_TESTING_OUTPUT_DIR_PATH,
            &objects,
            &arrow_registry,
        ),
        || re_types_builder::generate_docs(
            &reporter,
            DOC_CONTENT_DIR_PATH,
            &objects,
            &arrow_registry
        ),
    );

    report.finalize();

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}
