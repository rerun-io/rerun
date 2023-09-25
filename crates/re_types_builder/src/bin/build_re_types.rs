//! Helper binary for running the codegen manually. Useful during development!

use camino::Utf8Path;

const DEFINITIONS_DIR_PATH: &str = "crates/re_types/definitions";
const ENTRYPOINT_PATH: &str = "crates/re_types/definitions/rerun/archetypes.fbs";
const CPP_OUTPUT_DIR_PATH: &str = "rerun_cpp";
const RUST_OUTPUT_DIR_PATH: &str = "crates/re_types/.";
const PYTHON_OUTPUT_DIR_PATH: &str = "rerun_py/rerun_sdk/rerun";
const PYTHON_TESTING_OUTPUT_DIR_PATH: &str = "rerun_py/tests/test_types";
const DOCS_CONTENT_DIR_PATH: &str = "docs/content/reference/data_types_test";

macro_rules! join {
    ($($task:expr,)*) => {join!($($task),*)};
    ($($task:expr),*) => {{
        #![allow(clippy::redundant_closure_call)]
        ::rayon::scope(|scope| {
            $(scope.spawn(|_| ($task)());)*
        })
    }}
}

fn main() {
    re_log::setup_native_logging();

    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon-{i}"))
        .build_global()
        .unwrap();

    let mut profiler = re_tracing::Profiler::default();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" => {
                println!("Usage: [--help] [--profile]");
                return;
            }
            "--profile" => profiler.start(),
            _ => {
                eprintln!("Unknown argument: {arg:?}");
                return;
            }
        }
    }

    let workspace_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap();

    let definitions_dir_path = workspace_dir.join(DEFINITIONS_DIR_PATH);
    let entrypoint_path = workspace_dir.join(ENTRYPOINT_PATH);
    let cpp_output_dir_path = workspace_dir.join(CPP_OUTPUT_DIR_PATH);
    let rust_output_dir_path = workspace_dir.join(RUST_OUTPUT_DIR_PATH);
    let python_output_dir_path = workspace_dir.join(PYTHON_OUTPUT_DIR_PATH);
    let python_testing_output_dir_path = workspace_dir.join(PYTHON_TESTING_OUTPUT_DIR_PATH);
    let docs_content_dir_path = workspace_dir.join(DOCS_CONTENT_DIR_PATH);

    re_log::info!("Running codegen…");
    let (report, reporter) = re_types_builder::report::init();

    let (objects, arrow_registry) =
        re_types_builder::generate_lang_agnostic(definitions_dir_path, entrypoint_path);

    re_tracing::profile_scope!("Language-specific code-gen");
    join!(
        || re_types_builder::generate_cpp_code(
            &reporter,
            cpp_output_dir_path,
            &objects,
            &arrow_registry
        ),
        || re_types_builder::generate_rust_code(
            &reporter,
            rust_output_dir_path,
            &objects,
            &arrow_registry
        ),
        || re_types_builder::generate_python_code(
            &reporter,
            python_output_dir_path,
            python_testing_output_dir_path,
            &objects,
            &arrow_registry,
        ),
        || re_types_builder::generate_docs(
            &reporter,
            docs_content_dir_path,
            &objects,
            &arrow_registry
        ),
    );

    report.panic_on_errors();

    re_log::info!("Done.");
}
