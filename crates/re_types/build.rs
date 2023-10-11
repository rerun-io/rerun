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

        // No - we run a manual `cargo codegen` on CI in `.github/workflows/contrib_checks.yml`
        // (`no-codegen-changes`) to check out that the generated files are in-sync with the input.
        Environment::CI => false,

        Environment::DeveloperInWorkspace => {
            // This `build.rs` depends on having a bunch of tools installed (`clang-format`, …)
            // and when some random contributor clones our repository,
            // they likely won't have it, and we shouldn't need it.
            // We really only need this `build.rs` for the convenience of
            // developers who changes the input files (*.fbs) who then don't want to manually
            // run `cargo codegen`.
            // So: we only run this `build.rs` automatically after a developer
            // has once run the codegen MANUALLY first using `cargo codegen`.
            // That will produce the `source_hash.txt` file.

            Path::new(SOURCE_HASH_PATH).exists()
        }

        // We ship pre-built source files for users
        Environment::UsedAsDependency => false,
    }
}

fn main() {
    if !should_run() {
        return;
    }

    if re_build_tools::get_and_track_env_var("CARGO_FEATURE___OPT_OUT_OF_AUTO_REBUILD").is_ok() {
        eprintln!("__opt_out_of_auto_rebuild feature detected: Skipping re_types/build.rs");
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
