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
}

impl Build {
    pub fn run(self) -> anyhow::Result<()> {
        let Self {
            index_name,
            meilisearch_url,
            meilisearch_master_key,
            release_version,
        } = self;
        let client = meili::connect(&meilisearch_url, &meilisearch_master_key)?;
        let documents = ingest::run(release_version)?;
        client.index(&index_name, &documents)?;
        Ok(())
    }
}
