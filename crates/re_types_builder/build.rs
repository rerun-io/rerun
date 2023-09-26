//! Generates flatbuffers reflection code from `reflection.fbs`.

use std::path::Path;

use xshell::{cmd, Shell};

use re_build_tools::{
    compute_file_hash, is_tracked_env_var_set, read_versioning_hash, write_versioning_hash,
};

// ---

// NOTE: Don't need to add extra context to xshell invocations, it does so on its own.

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const FBS_REFLECTION_DEFINITION_PATH: &str = "./definitions/reflection.fbs";

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

    // We're building an actual build graph here, and Cargo has no idea about it.
    //
    // Worse: some nodes in our build graph actually output artifacts into the src/ directory,
    // which Cargo always interprets as "need to rebuild everything ASAP", leading to an infinite
    // feedback loop.
    //
    // For these reasons, we manually compute and track signature hashes for the graph nodes we
    // depend on, and make sure to exit early if everything's already up to date.
    let cur_hash = read_versioning_hash(SOURCE_HASH_PATH);
    let new_hash = compute_file_hash(FBS_REFLECTION_DEFINITION_PATH);

    // Leave these be please, very useful when debugging.
    eprintln!("cur_hash: {cur_hash:?}");
    eprintln!("new_hash: {new_hash:?}");

    if cur_hash.is_none() || cur_hash.as_ref() == Some(&new_hash) {
        // Source definition hasn't changed, no need to do anything.
        return;
    }

    // NOTE: This requires `flatc` to be in $PATH, but only for contributors, not end users.
    // Even for contributors, `flatc` won't be needed unless they edit some of the .fbs files.
    let sh = Shell::new().unwrap();
    cmd!(
        sh,
        "flatc -o src/ --rust --gen-onefile --filename-suffix '' {FBS_REFLECTION_DEFINITION_PATH}"
    )
    .run()
    .unwrap();

    // NOTE: We're purposefully ignoring the error here.
    //
    // In the very unlikely chance that the user doesn't have the `fmt` component installed,
    // there's still no good reason to fail the build.
    //
    // The CI will catch the unformatted file at PR time and complain appropriately anyhow.
    cmd!(sh, "cargo fmt").run().ok();

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}
