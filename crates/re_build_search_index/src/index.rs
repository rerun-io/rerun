use argh::FromArgs;

use crate::{ingest, meili, DEFAULT_KEY, DEFAULT_URL};

/// Index documentation, examples, and API references for all languages
#[derive(FromArgs)]
#[argh(subcommand, name = "index")]
pub struct Index {
    /// name of the meilisearch index to create/query
    #[argh(positional)]
    index_name: String,

    /// meilisearch URL
    #[argh(option, long = "url", default = "DEFAULT_URL.into()")]
    meilisearch_url: String,

    /// meilisearch master key (must support both read and write)
    #[argh(option, long = "master-key", default = "DEFAULT_KEY.into()")]
    meilisearch_master_key: String,
}

impl Index {
    pub async fn run(self) -> anyhow::Result<()> {
        let client = meili::connect(&self.meilisearch_url, &self.meilisearch_master_key).await?;
        let documents = ingest::run()?;
        client.index(&self.index_name, &documents).await?;
        Ok(())
    }
}
