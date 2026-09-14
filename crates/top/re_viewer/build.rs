// The build script always runs on the host, where the filesystem is available,
// even when the crate itself is compiled for the web.
#![allow(clippy::disallowed_methods)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// File extensions to embed from `agent_context`.
const AGENT_SOURCE_FILTER: &[&str] = &[
    "cpp", "h", "hpp", "in", "md", "py", "rs", "toml", "txt", "yaml",
];

fn main() {
    cfg_aliases::cfg_aliases! {
        agent_panel: { not(target_arch = "wasm32") },
    }

    re_build_tools::export_build_info_vars_for_crate("re_viewer");

    // The context is a couple of megabytes of documentation, and the web viewer has no
    // agent panel to hand it to.
    let embed = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() != "wasm32"
        && std::env::var_os("CARGO_FEATURE_AGENT_CONTEXT").is_some();
    embed_agent_files(embed);
}

/// Writes `$OUT_DIR/agent_files.rs` holding every matching file from `agent_context` as
/// `(path relative to the agent directory, contents)`.
///
/// The list is empty unless `embed`, so that the module compiles either way.
fn embed_agent_files(embed: bool) {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_dir = crate_root.join("agent_context");
    re_build_tools::rerun_if_changed_or_doesnt_exist(&source_dir);

    let mut files = Vec::new();
    if embed && source_dir.is_dir() {
        collect_files(crate_root, &source_dir, &mut files);
    }
    files.sort();

    if embed && files.is_empty() {
        // `agent_context/docs` and `agent_context/skills` are symlinks out of the crate, and
        // `cargo package` drops symlinked directories without a word. Say so, rather than
        // shipping an agent panel that has quietly lost its skills and documentation.
        println!(
            "cargo::warning=No agent context found in {}; \
             the agent panel will run without Rerun skills or documentation. \
             Build with `--no-default-features` or without the `agent_context` feature to \
             make this deliberate.",
            source_dir.display()
        );
    }

    let mut code = String::from("pub const AGENT_FILES: &[(&str, &[u8])] = &[\n");
    for (relative_path, absolute_path) in files {
        re_build_tools::rerun_if_changed(&absolute_path);
        writeln!(
            code,
            "    ({relative_path:?}, include_bytes!({:?})),",
            absolute_path.display()
        )
        .ok();
    }
    code.push_str("];\n");

    let out_dir = match std::env::var("OUT_DIR") {
        Ok(out_dir) => PathBuf::from(out_dir),
        Err(err) => panic!("OUT_DIR is not set: {err}"),
    };
    if let Err(err) =
        re_build_tools::write_file_if_necessary(out_dir.join("agent_files.rs"), code.as_bytes())
    {
        panic!("Failed to write agent_files.rs: {err}");
    }
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
            continue;
        }
        let extension = path.extension().map(|ext| ext.to_string_lossy());
        let wanted = extension.is_some_and(|ext| AGENT_SOURCE_FILTER.contains(&ext.as_ref()));
        if wanted && let Ok(relative) = path.strip_prefix(root) {
            let relative = relative.to_string_lossy().replace('\\', "/");
            files.push((relative, path));
        }
    }
}
