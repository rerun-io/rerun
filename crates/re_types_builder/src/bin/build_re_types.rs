//! Helper binary for running the codegen manually. Useful during development!

const DEFINITIONS_DIR_PATH: &str = "crates/re_types/definitions";
const ENTRYPOINT_PATH: &str = "crates/re_types/definitions/rerun/archetypes.fbs";
const CPP_OUTPUT_DIR_PATH: &str = "rerun_cpp/src/rerun";
const RUST_OUTPUT_DIR_PATH: &str = "crates/re_types/.";
const PYTHON_OUTPUT_DIR_PATH: &str = "rerun_py/rerun_sdk/rerun/_rerun2";

fn main() {
    re_log::setup_native_logging();

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

    re_log::info!("Running codegen…");
    let (objects, arrow_registry) =
        re_types_builder::generate_lang_agnostic(DEFINITIONS_DIR_PATH, ENTRYPOINT_PATH);

    re_tracing::profile_scope!("Language-specific code-gen");
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

    re_log::info!("Done.");
}

// Do 3 things in parallel
fn join3(a: impl FnOnce() + Send, b: impl FnOnce() + Send, c: impl FnOnce() + Send) {
    rayon::join(a, || rayon::join(b, c));
}
