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
const DOC_EXAMPLES_DIR_PATH: &str = "../../docs/code-examples";
const RUST_OUTPUT_DIR_PATH: &str = ".";
const PYTHON_OUTPUT_DIR_PATH: &str = "../../rerun_py/rerun_sdk/rerun/_rerun2";
const PYTHON_PYPROJECT_PATH: &str = "../../rerun_py/pyproject.toml";

// located in PYTHON_OUTPUT_DIR_PATH
const ARCHETYPE_OVERRIDES_SUB_DIR_PATH: &str = "archetypes/_overrides";
const COMPONENT_OVERRIDES_SUB_DIR_PATH: &str = "components/_overrides";
const DATATYPE_OVERRIDES_SUB_DIR_PATH: &str = "datatypes/_overrides";

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

    rerun_if_changed_or_doesnt_exist(SOURCE_HASH_PATH);
    for path in iter_dir(DEFINITIONS_DIR_PATH, Some(&["fbs"])) {
        rerun_if_changed(&path);
    }

    // NOTE: We need to hash both the flatbuffers definitions as well as the source code of the
    // code generator itself!
    let cur_hash = read_versioning_hash(SOURCE_HASH_PATH);
    let re_types_builder_hash = compute_crate_hash("re_types_builder");
    let definitions_hash = compute_dir_hash(DEFINITIONS_DIR_PATH, Some(&["fbs"]));
    let doc_examples_hash = compute_dir_hash(DOC_EXAMPLES_DIR_PATH, Some(&["rs", "py"]));
    let archetype_overrides_hash = compute_dir_hash(
        PathBuf::from(PYTHON_OUTPUT_DIR_PATH).join(ARCHETYPE_OVERRIDES_SUB_DIR_PATH),
        Some(&["py"]),
    );
    let component_overrides_hash = compute_dir_hash(
        PathBuf::from(PYTHON_OUTPUT_DIR_PATH).join(COMPONENT_OVERRIDES_SUB_DIR_PATH),
        Some(&["py"]),
    );
    let datatype_overrides_hash = compute_dir_hash(
        PathBuf::from(PYTHON_OUTPUT_DIR_PATH).join(DATATYPE_OVERRIDES_SUB_DIR_PATH),
        Some(&["py"]),
    );

    let new_hash = compute_strings_hash(&[
        &re_types_builder_hash,
        &definitions_hash,
        &doc_examples_hash,
        &archetype_overrides_hash,
        &component_overrides_hash,
        &datatype_overrides_hash,
    ]);

    // Leave these be please, very useful when debugging.
    eprintln!("re_types_builder_hash: {re_types_builder_hash:?}");
    eprintln!("definitions_hash: {definitions_hash:?}");
    eprintln!("doc_examples_hash: {doc_examples_hash:?}");
    eprintln!("archetype_overrides_hash: {archetype_overrides_hash:?}");
    eprintln!("component_overrides_hash: {component_overrides_hash:?}");
    eprintln!("datatype_overrides_hash: {datatype_overrides_hash:?}");
    eprintln!("new_hash: {new_hash:?}");
    eprintln!("cur_hash: {cur_hash:?}");

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
    if std::env::var("CI").is_ok() {
        panic!("re_types' fbs definitions and generated code are out-of-sync!");
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
    cmd!(sh, "cargo fmt -p re_types").run().ok();
    // RUN IT TWICE!!! `rustfmt` is __not__ idempotent until its second run!
    //
    // You can try it out yourself with e.g. this snippet:
    // ```
    // # [derive (Clone , Debug)]
    //
    // # [derive (Default , Copy , PartialEq , PartialOrd)]
    // pub struct Vec2D (pub [f32 ; 2usize] ,) ;
    // ```
    //
    // First run will take care of most things, but since `rustfmt` isn't recursive, it will also
    // miss the opportunity to merge the two #derive clauses after it took care of removing the
    // superfluous linefeeds:
    // ```
    // #[derive(Clone, Debug)]
    // #[derive(Default, Copy, PartialEq, PartialOrd)]
    // pub struct Vec2D(pub [f32; 2usize]);
    // ```
    //
    // Now if you run it a second time on the other hand...:
    // ```
    // #[derive(Clone, Debug, Default, Copy, PartialEq, PartialOrd)]
    // pub struct Vec2D(pub [f32; 2usize]);
    // ```
    //
    // And finally things are idempotent, for real this time.
    cmd!(sh, "cargo fmt -p re_types").run().ok();

    re_types_builder::generate_python_code(
        DEFINITIONS_DIR_PATH,
        PYTHON_OUTPUT_DIR_PATH,
        "./definitions/rerun/archetypes.fbs",
    );

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
        "black --config {PYTHON_PYPROJECT_PATH} {PYTHON_OUTPUT_DIR_PATH}"
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
        "ruff --config {PYTHON_PYPROJECT_PATH} --fix {PYTHON_OUTPUT_DIR_PATH}"
    )
    .run()
    .ok();

    write_versioning_hash(SOURCE_HASH_PATH, new_hash);
}
