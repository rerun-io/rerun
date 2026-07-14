//! Validates every `.wgsl` shader in the workspace at compile-test time.
//!
//! Our shaders are compiled lazily at runtime (the first time a pipeline that uses them is
//! created), so a broken shader could easily slip through code review and CI and only blow up
//! when a user happens to trigger that particular code path.
//!
//! This test resolves the `#import` directives the exact same way the renderer does at runtime
//! (via [`re_renderer::FileResolver`]) and then runs the resolved source through `naga`.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use re_renderer::{FileResolver, FileSystem, SearchPath};
use wgpu::naga;

/// A [`FileSystem`] backed by `std::fs`.
///
/// We can't reuse `re_renderer`'s own `OsFileSystem` because it's only compiled in for native
/// debug builds with shader hot-reloading enabled (`cfg(load_shaders_from_disk)`), which is not
/// guaranteed to be the case when running tests in CI.
struct DiskFileSystem;

impl FileSystem for DiskFileSystem {
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<Cow<'static, str>> {
        let path = path.as_ref();
        std::fs::read_to_string(path)
            .map(Into::into)
            .map_err(|err| anyhow::anyhow!("failed to read {path:?}: {err}"))
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref();
        std::fs::canonicalize(path)
            .map_err(|err| anyhow::anyhow!("failed to canonicalize {path:?}: {err}"))
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }
}

/// Recursively collects every `.wgsl` file under `root`.
fn collect_wgsl_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_wgsl_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "wgsl") {
            out.push(path);
        }
    }
}

#[test]
fn all_wgsl_shaders_are_valid() {
    // `CARGO_MANIFEST_DIR` points at the `re_renderer` crate directory.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let shader_dir = crate_dir.join("shader");
    // …/rerun (the workspace root that holds both `crates/` and `examples/`).
    let workspace_root = crate_dir
        .ancestors()
        .nth(3)
        .expect("re_renderer should live at crates/viewer/re_renderer");
    let examples_dir = workspace_root.join("examples");

    // Imports such as `#import <types.wgsl>` resolve against the search path; relative imports
    // (`#import <./utils/foo.wgsl>`) resolve against the importing file. The renderer's shaders
    // and the example shaders all import from `re_renderer/shader`, so that's the search root.
    let mut search_path = SearchPath::default();
    search_path.push(&shader_dir);
    let resolver = FileResolver::with_search_path(DiskFileSystem, search_path);

    let mut shaders = Vec::new();
    collect_wgsl_files(&shader_dir, &mut shaders);
    collect_wgsl_files(&examples_dir, &mut shaders);
    shaders.sort();
    assert!(
        !shaders.is_empty(),
        "found no .wgsl files to validate under {shader_dir:?} or {examples_dir:?}"
    );

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    let mut failures = Vec::new();
    let mut checked = 0;
    let mut skipped = 0;

    for shader in &shaders {
        let rel = shader.strip_prefix(workspace_root).unwrap_or(shader);

        // Resolve all `#import`s, producing a self-contained module.
        let interpolated = match resolver.populate(shader) {
            Ok(interpolated) => interpolated,
            Err(err) => {
                failures.push(format!(
                    "{}\n  failed to resolve imports: {err}",
                    rel.display()
                ));
                continue;
            }
        };

        // Files without an entry point are include-only fragments (type/util libraries). They
        // can't be validated standalone — they get validated transitively wherever they're
        // imported into an entry-point shader.
        let has_entry_point = ["@vertex", "@fragment", "@compute"]
            .iter()
            .any(|kw| interpolated.contents.contains(kw));
        if !has_entry_point {
            skipped += 1;
            continue;
        }

        let source = &interpolated.contents;

        let module = match naga::front::wgsl::parse_str(source) {
            Ok(module) => module,
            Err(err) => {
                failures.push(format!(
                    "{}\n{}",
                    rel.display(),
                    indent(&err.emit_to_string(source))
                ));
                continue;
            }
        };

        if let Err(err) = validator.validate(&module) {
            failures.push(format!(
                "{}\n{}",
                rel.display(),
                indent(&err.emit_to_string(source))
            ));
            continue;
        }

        checked += 1;
    }

    eprintln!(
        "Validated {checked} entry-point shader(s), skipped {skipped} include-only fragment(s)."
    );

    assert!(
        failures.is_empty(),
        "{} shader(s) failed validation:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
