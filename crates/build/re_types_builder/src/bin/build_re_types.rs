//! This binary runs the codegen manually.
//!
//! It is easiest to call this using `pixi run codegen`,
//! which will set up the necessary tools.

// TODO(#3408): remove unwrap()
#![expect(clippy::unwrap_used)]

use camino::Utf8Path;
use re_build_tools::{
    read_versioning_hash, set_output_cargo_build_instructions, write_versioning_hash,
};
use re_types_builder::{SourceLocations, compute_re_types_hash};

const RE_TYPES_SOURCE_HASH_PATH: &str = "crates/store/re_sdk_types/source_hash.txt";
const DEFINITIONS_DIR_PATH: &str = "crates/store/re_sdk_types/definitions";
const ENTRYPOINT_PATH: &str = "crates/store/re_sdk_types/definitions/entry_point.fbs";
const SNIPPETS_DIR_PATH: &str = "docs/snippets/all";
const CPP_OUTPUT_DIR_PATH: &str = "rerun_cpp";
const PYTHON_OUTPUT_DIR_PATH: &str = "rerun_py/rerun_sdk/rerun";
const PYTHON_TESTING_OUTPUT_DIR_PATH: &str = "rerun_py/tests/test_types";
const DOCS_CONTENT_DIR_PATH: &str = "docs/content/reference/types";
const SNIPPETS_REF_DIR_PATH: &str = "docs/snippets/";

/// This uses [`rayon::scope`] to spawn all closures as tasks
/// running in parallel. It blocks until all tasks are done.
macro_rules! join {
    ($($task:expr,)*) => {join!($($task),*)};
    ($($task:expr),*) => {{
        ::rayon::scope(|scope| {
            $(scope.spawn(|_| ($task)());)*
        })
    }}
}

fn main() {
    re_log::setup_logging();

    #[cfg(feature = "tracing")]
    let mut profiler = re_tracing::Profiler::default(); // must be started early and dropped late to catch everything

    // This isn't a build.rs script, so opt out of cargo build instructions
    set_output_cargo_build_instructions(false);

    let mut always_run = false;
    let mut check = false;
    let mut warnings_as_errors = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" => {
                println!("Usage: [--help] [--force] [--profile]");
                return;
            }
            "--force" => always_run = true,
            "--check" => {
                always_run = true;
                check = true;
            }
            "--warnings-as-errors" => warnings_as_errors = true,

            #[cfg(feature = "tracing")]
            "--profile" => profiler.start(),

            _ => {
                eprintln!("Unknown argument: {arg:?}");
                return;
            }
        }
    }

    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon-{i}"))
        .build_global()
        .unwrap();

    let workspace_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap();

    assert!(
        workspace_dir.join("CODE_OF_CONDUCT.md").exists(),
        "failed to find workspace root"
    );

    let re_types_source_hash_path = workspace_dir.join(RE_TYPES_SOURCE_HASH_PATH);
    let definitions_dir_path = workspace_dir.join(DEFINITIONS_DIR_PATH);
    let entrypoint_path = workspace_dir.join(ENTRYPOINT_PATH);
    let cpp_output_dir_path = workspace_dir.join(CPP_OUTPUT_DIR_PATH);
    let python_output_dir_path = workspace_dir.join(PYTHON_OUTPUT_DIR_PATH);
    let python_testing_output_dir_path = workspace_dir.join(PYTHON_TESTING_OUTPUT_DIR_PATH);
    let docs_content_dir_path = workspace_dir.join(DOCS_CONTENT_DIR_PATH);
    let snippets_ref_dir_path = workspace_dir.join(SNIPPETS_REF_DIR_PATH);

    let cur_hash = read_versioning_hash(&re_types_source_hash_path);
    re_log::debug!("cur_hash: {cur_hash:?}");

    let new_hash = compute_re_types_hash(&SourceLocations {
        definitions_dir: DEFINITIONS_DIR_PATH,
        snippets_dir: SNIPPETS_DIR_PATH,
        python_output_dir: PYTHON_OUTPUT_DIR_PATH,
        cpp_output_dir: CPP_OUTPUT_DIR_PATH,
    });

    if let Some(cur_hash) = cur_hash {
        if cur_hash == new_hash {
            if always_run {
                re_log::info!(
                    "The hash hasn't changed, but --force was passed, so we'll run anyway."
                );
            } else {
                re_log::info!("Returning early: no changes detected (and --force wasn't set).");
                return;
            }
        } else {
            re_log::info!("Change detected");
        }
    } else {
        re_log::info!("Missing {re_types_source_hash_path:?} (first time running codegen)");
    }

    re_log::info!("Running codegen…");
    let (report, reporter) = re_types_builder::report::init();

    re_log::info!("Generating flatbuffers code…");
    re_types_builder::generate_fbs(&reporter, &definitions_dir_path, check);

    let (objects, type_registry) =
        re_types_builder::generate_lang_agnostic(&reporter, definitions_dir_path, entrypoint_path);

    re_tracing::profile_scope!("Language-specific code-gen");
    join!(
        || re_types_builder::generate_cpp_code(
            &reporter,
            cpp_output_dir_path,
            &objects,
            &type_registry,
            check,
        ),
        || re_types_builder::generate_rust_code(
            &reporter,
            workspace_dir,
            &objects,
            &type_registry,
            check,
        ),
        || re_types_builder::generate_python_code(
            &reporter,
            python_output_dir_path,
            python_testing_output_dir_path,
            &objects,
            &type_registry,
            check,
        ),
        || re_types_builder::generate_docs(
            &reporter,
            docs_content_dir_path,
            &objects,
            &type_registry,
            check,
        ),
        || re_types_builder::generate_snippets_ref(
            &reporter,
            snippets_ref_dir_path,
            &objects,
            &type_registry,
            check,
        ),
    );

    report.finalize(warnings_as_errors);

    write_versioning_hash(re_types_source_hash_path, new_hash);

    re_log::info!("Done.");
}
