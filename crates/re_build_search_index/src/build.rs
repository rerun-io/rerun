use crate::{ingest, meili, DEFAULT_INDEX, DEFAULT_KEY, DEFAULT_URL};

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
}

impl Default for Build {
    fn default() -> Self {
        Self {
            index_name: DEFAULT_INDEX.into(),
            meilisearch_url: DEFAULT_URL.into(),
            meilisearch_master_key: DEFAULT_KEY.into(),
        }
    }
}

impl Build {
    pub async fn run(self) -> anyhow::Result<()> {
        let client = meili::connect(&self.meilisearch_url, &self.meilisearch_master_key).await?;
        let documents = ingest::run()?;
        client.index(&self.index_name, &documents).await?;
        Ok(())
    }
}
