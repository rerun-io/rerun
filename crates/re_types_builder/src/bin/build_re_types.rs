//! Helper binary for running the codegen manually. Useful during development!

use camino::Utf8Path;

const DEFINITIONS_DIR_PATH: &str = "crates/re_types/definitions";
const ENTRYPOINT_PATH: &str = "crates/re_types/definitions/rerun/archetypes.fbs";
const CPP_OUTPUT_DIR_PATH: &str = "rerun_cpp";
const RUST_OUTPUT_DIR_PATH: &str = "crates/re_types/.";
const PYTHON_OUTPUT_DIR_PATH: &str = "rerun_py/rerun_sdk/rerun";

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

    re_log::info!("Running codegen…");
    let (objects, arrow_registry) =
        re_types_builder::generate_lang_agnostic(definitions_dir_path, entrypoint_path);

    re_tracing::profile_scope!("Language-specific code-gen");
    join3(
        || re_types_builder::generate_cpp_code(cpp_output_dir_path, &objects, &arrow_registry),
        || re_types_builder::generate_rust_code(rust_output_dir_path, &objects, &arrow_registry),
        || {
            re_types_builder::generate_python_code(
                python_output_dir_path,
                &objects,
                &arrow_registry,
            );
        },
    );

    re_log::info!("Done.");
}

// Do 3 things in parallel
fn join3(a: impl FnOnce() + Send, b: impl FnOnce() + Send, c: impl FnOnce() + Send) {
    rayon::join(a, || rayon::join(b, c));
}
