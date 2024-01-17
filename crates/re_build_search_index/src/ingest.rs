mod docs;
mod doxygen;
mod examples;
mod griffe;
mod rustdoc;

use camino::Utf8Path;
use cargo_metadata::Package;
use std::cell::Cell;
use std::cell::RefCell;

pub fn run() -> anyhow::Result<Vec<Document>> {
    let ctx = Context::new()?;
    docs::ingest(&ctx)?;
    examples::ingest(&ctx)?;
    rustdoc::ingest(&ctx)?;
    // griffe::ingest(ctx)?;
    // doxygen::ingest(ctx)?;
    Ok(ctx.finish())
}

struct Context {
    metadata: cargo_metadata::Metadata,
    id_gen: IdGen,
    documents: RefCell<Vec<Document>>,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            metadata: re_build_tools::cargo_metadata()?,
            id_gen: IdGen::new(),
            documents: RefCell::new(Vec::new()),
        })
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
        println!("indexed {} [{}]", data.title, data.url);
        self.documents.borrow_mut().push(Document {
            id: self.id_gen.next(),
            data,
        });
    }

    fn finish(self) -> Vec<Document> {
        self.documents.into_inner()
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
