//! Generates flatbuffers reflection code from `reflection.fbs`.

use std::path::Path;

use re_build_tools::{compute_file_hash, read_versioning_hash, write_versioning_hash};
use xshell::{Shell, cmd};

// ---

// NOTE: Don't need to add extra context to xshell invocations, it does so on its own.

const SOURCE_HASH_PATH: &str = "./source_hash.txt";
const FBS_REFLECTION_DEFINITION_PATH: &str = "./definitions/reflection.fbs";

fn should_run() -> bool {
    #![expect(clippy::match_same_arms)]
    use re_build_tools::Environment;

    match Environment::detect() {
        // we should have been run before publishing
        Environment::PublishingCrates => false,

        // The code we're generating here is actual source code that gets committed into the repository.
        Environment::RerunCI | Environment::CondaBuild => false,

        Environment::DeveloperInWorkspace => {
            // This `build.rs` depends on having `flatc` installed,
            // and when some random contributor clones our repository,
            // they likely won't have it, and we shouldn't need it.
            // We really only need this `build.rs` for the convenience of
            // developers who changes the input file (reflection.fbs),
            // which again is rare.
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
    let sh = Shell::new().expect("Shell::new() failed");
    #[expect(clippy::unwrap_used)] // unwrap is okay here
    cmd!(
        sh,
        "flatc -o src/ --rust --gen-onefile --filename-suffix '' {FBS_REFLECTION_DEFINITION_PATH}"
    )
    .run()
    .map_err(|err| eprintln!("flatc failed with error: {err}"))
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
