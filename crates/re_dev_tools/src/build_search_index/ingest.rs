/// Docs read from `/docs`
mod docs;

/// Examples read from `/examples`
mod examples;

/// Rust API reference generated by rustdoc
mod rust;

/// Python API reference generated by mkdocs
mod python;

/// C++ API reference generated by Doxygen
mod cpp;

use camino::Utf8Path;
use cargo_metadata::semver::Version;
use cargo_metadata::Package;
use indicatif::MultiProgress;
use indicatif::ProgressBar;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::io::IsTerminal;
use std::time::Duration;

pub fn run(
    release_version: Option<Version>,
    _exclude_crates: &[String],
) -> anyhow::Result<Vec<Document>> {
    let ctx = Context::new(release_version)?;
    docs::ingest(&ctx)?;
    examples::ingest(&ctx)?;
    // rust::ingest(&ctx, exclude_crates)?;
    python::ingest(&ctx)?;
    cpp::ingest(&ctx)?;
    Ok(ctx.finish())
}

struct Context {
    progress: MultiProgress,
    metadata: cargo_metadata::Metadata,
    id_gen: IdGen,
    documents: RefCell<Vec<Document>>,
    release_version: Option<Version>,
    is_tty: bool,
}

impl Context {
    fn new(release_version: Option<Version>) -> anyhow::Result<Self> {
        Ok(Self {
            progress: MultiProgress::new(),
            metadata: re_build_tools::cargo_metadata()?,
            id_gen: IdGen::new(),
            documents: RefCell::new(Vec::new()),
            release_version,
            is_tty: std::io::stdout().is_terminal(),
        })
    }

    fn is_tty(&self) -> bool {
        self.is_tty
    }

    fn progress_bar(&self, prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
        let bar = ProgressBar::new_spinner().with_prefix(prefix);
        bar.enable_steady_tick(Duration::from_millis(100));
        bar.set_style(bar.style().template("{spinner} {prefix}: {msg}").unwrap());
        self.progress.add(bar)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn finish_progress_bar(&self, bar: ProgressBar) {
        bar.disable_steady_tick();
        bar.finish_and_clear();
        self.progress.remove(&bar);
        let _ = bar;
    }

    fn workspace_root(&self) -> &Utf8Path {
        &self.metadata.workspace_root
    }

    fn rerun_pkg(&self) -> &Package {
        self.metadata
            .packages
            .iter()
            .find(|pkg| pkg.name == "rerun")
            .unwrap()
    }

    fn release_version(&self) -> &Version {
        self.release_version
            .as_ref()
            .unwrap_or_else(|| &self.rerun_pkg().version)
    }

    fn push(&self, data: DocumentData) {
        self.documents.borrow_mut().push(Document {
            id: self.id_gen.next(),
            data,
        });
    }

    fn finish(self) -> Vec<Document> {
        let documents = self.documents.into_inner();
        println!("collected {} documents", documents.len());
        documents
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Document {
    id: u64,
    #[serde(flatten)]
    data: DocumentData,
}

impl Document {
    pub const PRIMARY_KEY: &'static str = "id";

    pub fn title(&self) -> &String {
        &self.data.title
    }

    pub fn url(&self) -> &String {
        &self.data.url
    }

    pub fn content(&self) -> &String {
        &self.data.content
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DocumentData {
    kind: DocumentKind,
    title: String,
    hidden_tags: Vec<String>,
    tags: Vec<String>,
    content: String,
    url: String,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum DocumentKind {
    Docs,
    Examples,
    Rust,
    Python,
    Cpp,
}

struct IdGen {
    v: Cell<u64>,
}

impl IdGen {
    fn new() -> Self {
        Self { v: Cell::new(0) }
    }

    fn next(&self) -> u64 {
        self.v.replace(self.v.get() + 1)
    }
}
