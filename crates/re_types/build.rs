//! Generates Rust & Python code from flatbuffers definitions.

use std::path::PathBuf;

use xshell::{cmd, Shell};

use re_build_tools::{
    compute_crate_hash, compute_dir_hash, compute_strings_hash, is_tracked_env_var_set, iter_dir,
    read_versioning_hash, rerun_if_changed, rerun_if_changed_or_doesnt_exist,
    write_versioning_hash,
};

// NOTE: Don't need to add extra context to xshell invocations, it does so on its own.

// ---

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const DEFINITIONS_DIR_PATH: &str = "./definitions";
const RUST_OUTPUT_DIR_PATH: &str = ".";
const PYTHON_OUTPUT_DIR_PATH: &str = "../../rerun_py/rerun_sdk/rerun/rerun2";

fn main() {
    if std::env::var("CI").is_ok() {
        // Don't run on CI!
        //
        // The code we're generating here is actual source code that gets committed into the
        // repository.
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
    for path in iter_dir(DEFINITIONS_DIR_PATH, Some(&[".fbs"])) {
        rerun_if_changed(&path);
    }

    // NOTE: We need to hash both the flatbuffers definitions as well as the source code of the
    // code generator itself!
    let cur_hash = read_versioning_hash(SOURCE_HASH_PATH);
    let re_types_builder_hash = compute_crate_hash("re_types_builder");
    let definitions_hash = compute_dir_hash(DEFINITIONS_DIR_PATH, Some(&[".fbs"]));
    let new_hash = compute_strings_hash(&[&re_types_builder_hash, &definitions_hash]);

    // Leave these be please, very useful when debugging.
    eprintln!("re_types_builder_hash: {re_types_builder_hash:?}");
    eprintln!("cur_hash: {cur_hash:?}");
    eprintln!("definitions_hash: {definitions_hash:?}");
    eprintln!("new_hash: {new_hash:?}");

    if let Some(cur_hash) = cur_hash {
        if cur_hash == new_hash {
            // Neither the source of the code generator nor the IDL definitions have changed, no need
            // to do anything at this point.
            return;
        }
    }

    let sh = Shell::new().unwrap();

    re_types_builder::generate_rust_code(
        DEFINITIONS_DIR_PATH,
        RUST_OUTPUT_DIR_PATH,
        "./definitions/rerun/archetypes.fbs",
    );

    // NOTE: We're purposefully ignoring the error here.
    //
    // In the very unlikely chance that the user doesn't have the `fmt` component installed,
    // there's still no good reason to fail the build.
    //
    // The CI will catch the unformatted file at PR time and complain appropriately anyhow.
    cmd!(sh, "cargo fmt").run().ok();

    re_types_builder::generate_python_code(
        DEFINITIONS_DIR_PATH,
        PYTHON_OUTPUT_DIR_PATH,
        "./definitions/rerun/archetypes.fbs",
    );

    let pyproject_path = PathBuf::from(PYTHON_OUTPUT_DIR_PATH)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("pyproject.toml")
        .to_string_lossy()
        .to_string();

    // NOTE: This requires both `black` and `ruff` to be in $PATH, but only for contributors,
    // not end users.
    // Even for contributors, `black` and `ruff` won't be needed unless they edit some of the
    // .fbs files... and even then, this won't crash if they are missing, it will just fail to pass
    // the CI!

    // NOTE: We're purposefully ignoring the error here.
    //
    // If the user doesn't have `black` in their $PATH, there's still no good reason to fail
    // the build.
    //
    // The CI will catch the unformatted files at PR time and complain appropriately anyhow.
    cmd!(
        sh,
        "black --config {pyproject_path} {PYTHON_OUTPUT_DIR_PATH}"
    )
    .run()
    .ok();

    // NOTE: We're purposefully ignoring the error here.
    //
    // If the user doesn't have `ruff` in their $PATH, there's still no good reason to fail
    // the build.
    //
    // The CI will catch the unformatted files at PR time and complain appropriately anyhow.
    cmd!(
        sh,
        "ruff --config {pyproject_path} --fix {PYTHON_OUTPUT_DIR_PATH}"
    )
    .run()
    .ok();

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}
