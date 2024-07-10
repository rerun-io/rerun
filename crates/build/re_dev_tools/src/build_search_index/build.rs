use super::{ingest, meili, DEFAULT_INDEX, DEFAULT_KEY, DEFAULT_URL};
use cargo_metadata::semver::Version;

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
}

impl Build {
    pub fn run(self) -> anyhow::Result<()> {
        let client = meili::connect(&self.meilisearch_url, &self.meilisearch_master_key)?;
        let documents = ingest::run(self.release_version, &self.exclude_crates)?;
        client.index(&self.index_name, &documents)?;
        Ok(())
    }
}
