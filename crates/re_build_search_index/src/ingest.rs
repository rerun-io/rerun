mod docs;
mod doxygen;
mod examples;
mod griffe;
mod rustdoc;

use camino::Utf8Path;
use cargo_metadata::Package;
use indicatif::MultiProgress;
use indicatif::ProgressBar;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::time::Duration;

pub fn run() -> anyhow::Result<Vec<Document>> {
    let ctx = Context::new()?;
    docs::ingest(&ctx)?;
    examples::ingest(&ctx)?;
    rustdoc::ingest(&ctx)?;
    // griffe::ingest(&ctx)?;
    // doxygen::ingest(ctx)?;
    Ok(ctx.finish())
}

struct Context {
    progress: MultiProgress,
    metadata: cargo_metadata::Metadata,
    id_gen: IdGen,
    documents: RefCell<Vec<Document>>,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            progress: MultiProgress::new(),
            metadata: re_build_tools::cargo_metadata()?,
            id_gen: IdGen::new(),
            documents: RefCell::new(Vec::new()),
        })
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

    fn push(&self, data: DocumentData) {
        self.documents.borrow_mut().push(Document {
            id: self.id_gen.next(),
            data,
        });
    }

    fn finish(self) -> Vec<Document> {
        let documents = self.documents.into_inner();
        println!("indexed {} documents", documents.len());
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
    pub fn title(&self) -> &String {
        &self.data.title
    }

    pub fn url(&self) -> &String {
        &self.data.url
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DocumentData {
    kind: DocumentKind,
    title: String,
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
