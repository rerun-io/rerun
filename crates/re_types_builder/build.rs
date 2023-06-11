//! Generates flatbuffers reflection code from `reflection.fbs`.

use xshell::{cmd, Shell};

use re_build_tools::{
    compute_file_hash, is_tracked_env_var_set, read_versioning_hash, rerun_if_changed,
    rerun_if_changed_or_doesnt_exist, write_versioning_hash,
};

// ---

// NOTE: Don't need to add extra context to xshell invocations, it does so on its own.

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const FBS_REFLECTION_DEFINITION_PATH: &str = "./definitions/reflection.fbs";

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
    rerun_if_changed(FBS_REFLECTION_DEFINITION_PATH);

    let cur_hash = read_versioning_hash(SOURCE_HASH_PATH);
    let new_hash = compute_file_hash(FBS_REFLECTION_DEFINITION_PATH);

    // Leave these be please, very useful when debugging.
    eprintln!("cur_hash: {cur_hash:?}");
    eprintln!("new_hash: {new_hash:?}");

    if let Some(cur_hash) = cur_hash {
        if cur_hash == new_hash {
            // Source definition hasn't changed, no need to do anything.
            return;
        }
    }

    let sh = Shell::new().unwrap();
    cmd!(
        sh,
        "flatc -o src/ --rust --gen-onefile --filename-suffix '' {FBS_REFLECTION_DEFINITION_PATH}"
    )
    .run()
    .unwrap();

    // NOTE: We're purposefully ignoring the error here.
    //
    // In the very unlikely chance that the user doesn't have `rustfmt` in their $PATH, there's
    // still no good reason to fail the build.
    //
    // The CI will catch the unformatted file at PR time and complain appropriately anyhow.
    cmd!(sh, "cargo fmt").run().ok();

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}
