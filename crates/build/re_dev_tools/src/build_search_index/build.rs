use cargo_metadata::semver::Version;

use super::{DEFAULT_INDEX, DEFAULT_KEY, DEFAULT_URL, ingest, meili};

/// Index documentation, examples, and API references for all languages
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "build")]
pub struct Build {
    /// name of the meilisearch index to create/query
    #[argh(positional, default = "DEFAULT_INDEX.into()")]
    index_name: String,

    /// meilisearch URL
    #[argh(option, long = "url", default = "DEFAULT_URL.into()")]
    meilisearch_url: String,

    /// meilisearch master key (must support both read and write)
    #[argh(option, long = "master-key", default = "DEFAULT_KEY.into()")]
    meilisearch_master_key: String,

    /// release version to use in URLs
    #[argh(option, long = "release-version")]
    release_version: Option<Version>,

    /// exclude one or more crates
    #[argh(option, long = "exclude-crate")]
    exclude_crates: Vec<String>,

    /// rust toolchain version, e.g. nightly-2025-02-05
    #[argh(option, long = "rust-toolchain")]
    rust_toolchain: Option<String>,
}

impl Build {
    pub fn run(self) -> anyhow::Result<()> {
        let client = meili::connect(&self.meilisearch_url, &self.meilisearch_master_key)?;
        let documents = ingest::run(
            self.release_version,
            &self.exclude_crates,
            self.rust_toolchain.as_deref().unwrap_or("nightly"),
        )?;
        client.index(&self.index_name, &documents)?;
        Ok(())
    }
}
