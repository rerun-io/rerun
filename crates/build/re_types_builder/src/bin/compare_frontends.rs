//! Asserts that the Flatbuffers and Rust frontends describe the same types.
//!
//! This is the spine of the migration off Flatbuffers: the two definition trees are kept side by
//! side until the flip, and this is what says they still mean the same thing. It runs both
//! frontends over their own tree and then the whole of codegen over each result, and requires the
//! Rust, Python, C++ and docs that come out to be byte-identical.
//!
//! Generated code is what we compare because it is a total function of the IR — anything the
//! frontends disagree about that no backend reads cannot break the SDKs, and anything a backend
//! does read shows up here without a hand-written comparison having to know about it.
//!
//! Easiest called as `pixi run compare-frontends`.
//!
//! TODO(RR-5384): remove once we've migrated completely from flatbuffers.

// TODO(#3408): remove unwrap()
#![expect(clippy::unwrap_used)]

use camino::Utf8Path;

use re_types_builder::{
    CodeGenerator, CppCodeGenerator, DocsCodeGenerator, GeneratedFiles, Objects,
    PythonCodeGenerator, Reporter, RustCodeGenerator, SnippetsRefCodeGenerator, TypeRegistry,
};

const DEFINITIONS_DIR_PATH: &str = "crates/store/re_sdk_types/definitions";
const ENTRYPOINT_PATH: &str = "crates/store/re_sdk_types/definitions/entry_point.fbs";
const CPP_OUTPUT_DIR_PATH: &str = "rerun_cpp";
const PYTHON_OUTPUT_DIR_PATH: &str = "rerun_py/rerun_sdk/rerun";
const PYTHON_TESTING_OUTPUT_DIR_PATH: &str = "rerun_py/tests/test_types";
const DOCS_CONTENT_DIR_PATH: &str = "docs/content/reference/types";
const SNIPPETS_REF_DIR_PATH: &str = "docs/snippets/";

fn main() {
    re_log::setup_logging();

    let workspace_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap();

    assert!(
        workspace_dir.join("CODE_OF_CONDUCT.md").exists(),
        "failed to find workspace root"
    );

    let definitions_dir = workspace_dir.join(DEFINITIONS_DIR_PATH);
    let entrypoint = workspace_dir.join(ENTRYPOINT_PATH);

    let (report, reporter) = re_types_builder::report::init();

    re_log::info!("Reading the Flatbuffers definitions…");
    let (from_fbs, fbs_registry) =
        re_types_builder::generate_lang_agnostic(&reporter, &definitions_dir, &entrypoint);

    re_log::info!("Reading the Rust definitions…");
    let (from_rust, rust_registry) = rust_frontend(&reporter, &definitions_dir);

    re_log::info!("Generating both SDKs…");
    let fbs_files = generate_everything(&reporter, workspace_dir, &from_fbs, &fbs_registry);
    let rust_files = generate_everything(&reporter, workspace_dir, &from_rust, &rust_registry);

    compare(&reporter, &fbs_files, &rust_files);

    report.finalize(false);

    re_log::info!("The two frontends agree, across {} files.", fbs_files.len());
}

fn rust_frontend(reporter: &Reporter, definitions_dir: &Utf8Path) -> (Objects, TypeRegistry) {
    let mut objects = Objects::from_rust_definitions(reporter, definitions_dir);

    // The only place a definition's own path reaches generated code is the `// Based on …` header,
    // and until the flip the two frontends read different files. Everything else has to match.
    for object in objects.objects.values_mut() {
        object.virtpath = as_fbs(&object.virtpath);
        object.filepath = as_fbs(object.filepath.as_str()).into();
        for field in &mut object.fields {
            field.virtpath = as_fbs(&field.virtpath);
            field.filepath = as_fbs(field.filepath.as_str()).into();
        }
    }

    let mut type_registry = TypeRegistry::default();
    for object in objects.objects.values_mut() {
        type_registry.register(object);
    }

    (objects, type_registry)
}

/// `…/components/position3d.def.rs` -> `…/components/position3d.fbs`.
fn as_fbs(path: &str) -> String {
    match path.strip_suffix(".def.rs") {
        Some(without_suffix) => format!("{without_suffix}.fbs"),
        None => path.to_owned(),
    }
}

/// Runs every backend, in memory.
///
/// Formatting is skipped: it is a function of the generated text, so two runs that agree here
/// agree after formatting too.
fn generate_everything(
    reporter: &Reporter,
    workspace_dir: &Utf8Path,
    objects: &Objects,
    type_registry: &TypeRegistry,
) -> GeneratedFiles {
    let mut generators: Vec<Box<dyn CodeGenerator>> = vec![
        Box::new(CppCodeGenerator::new(
            workspace_dir.join(CPP_OUTPUT_DIR_PATH),
        )),
        Box::new(RustCodeGenerator::new(workspace_dir)),
        Box::new(PythonCodeGenerator::new(
            workspace_dir.join(PYTHON_OUTPUT_DIR_PATH),
            workspace_dir.join(PYTHON_TESTING_OUTPUT_DIR_PATH),
        )),
        Box::new(DocsCodeGenerator::new(
            workspace_dir.join(DOCS_CONTENT_DIR_PATH),
        )),
        Box::new(SnippetsRefCodeGenerator::new(
            workspace_dir.join(SNIPPETS_REF_DIR_PATH),
        )),
    ];

    let mut files = GeneratedFiles::default();
    for generator in &mut generators {
        files.extend(generator.generate(reporter, objects, type_registry));
    }
    files
}

fn compare(reporter: &Reporter, from_fbs: &GeneratedFiles, from_rust: &GeneratedFiles) {
    let mut differing_headers = 0;

    for (path, fbs_contents) in from_fbs {
        let Some(rust_contents) = from_rust.get(path) else {
            reporter.error_file(path, "Only the Flatbuffers frontend generates this file");
            continue;
        };

        if fbs_contents == rust_contents {
            continue;
        }

        let fbs_body = without_source_header(fbs_contents);
        let rust_body = without_source_header(rust_contents);

        if fbs_body == rust_body {
            differing_headers += 1;
        } else {
            reporter.error_file(path, format!("Differs:\n{}", diff(&fbs_body, &rust_body)));
        }
    }

    for path in from_rust.keys() {
        if !from_fbs.contains_key(path) {
            reporter.error_file(path, "Only the Rust frontend generates this file");
        }
    }

    if 0 < differing_headers {
        re_log::info!(
            "{differing_headers} files differ only in their `// Based on …` header, which is \
             expected while both definition trees exist."
        );
    }
}

/// Strips the `// Based on "…"` line naming the definition the file was generated from.
///
/// This is the one thing the two trees are allowed to disagree about: a Rust definition's path is
/// its package, and a handful of the Flatbuffers files sit in a directory that does not match the
/// namespace they declare. The flip regenerates every header in one go.
fn without_source_header(contents: &str) -> String {
    const HEADER: &str = "Based on \"crates/store/re_sdk_types/definitions/";

    contents
        .lines()
        .filter(|line| !line.contains(HEADER))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The first few differing lines, with a little context.
fn diff(fbs: &str, rust: &str) -> String {
    const CONTEXT: usize = 3;

    let fbs: Vec<&str> = fbs.lines().collect();
    let rust: Vec<&str> = rust.lines().collect();

    let first = std::iter::zip(&fbs, &rust)
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| fbs.len().min(rust.len()));

    let show = |lines: &[&str], what: &str| {
        let start = first.saturating_sub(CONTEXT);
        let end = (first + CONTEXT).min(lines.len());
        let body = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("  {:>5} | {line}", start + i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{what}:\n{body}")
    };

    format!("{}\n{}", show(&fbs, "fbs"), show(&rust, "rust"))
}
