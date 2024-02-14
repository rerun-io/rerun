use anyhow::Context;
use meilisearch_sdk::{Client, Error, ErrorCode, MeilisearchError, Task};

use crate::ingest::Document;

pub async fn connect(url: &str, master_key: &str) -> anyhow::Result<SearchClient> {
    SearchClient::connect(url, master_key).await
}

pub struct SearchClient {
    client: Client,
}

impl SearchClient {
    /// Connect to Meilisearch.
    ///
    /// `master_key` can be obtained via the Meilisearch could console,
    /// or set via the `--master-key` option when running a local instance.
    pub async fn connect(url: &str, master_key: &str) -> anyhow::Result<Self> {
        let client = Client::new(url, Some(master_key));
        // this call can only be done with a valid master key
        let _ = client
            .get_keys()
            .await
            .with_context(|| format!("cannot connect to meilisearch at {url:?}"))?;
        Ok(Self { client })
    }

    /// Create an index from `documents`.
    ///
    /// If an index with the same name already exists, it is deleted first.
    pub async fn index(&self, index: &str, documents: &[Document]) -> anyhow::Result<()> {
        if self.index_exists(index).await? {
            self.delete_index(index).await?; // delete existing index
        }
        self.create_index(index, documents).await
    }

    /// Query a specific index in the database.
    pub async fn query(
        &self,
        index: &str,
        q: &str,
        limit: Option<usize>,
    ) -> anyhow::Result<impl Iterator<Item = Document> + DoubleEndedIterator + ExactSizeIterator>
    {
        let index = self.client.index(index);
        let mut request = index.search();
        let request = request.with_query(q);
        request.limit = limit;
        let results = request.execute().await?;
        Ok(results.hits.into_iter().map(|hit| hit.result))
    }

    async fn index_exists(&self, index: &str) -> anyhow::Result<bool> {
        match self.client.get_index(index).await {
            Ok(_) => Ok(true),
            Err(Error::Meilisearch(MeilisearchError {
                error_code: ErrorCode::IndexNotFound,
                ..
            })) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    async fn create_index(&self, index: &str, documents: &[Document]) -> anyhow::Result<()> {
        self.client
            .index(index)
            .add_or_update(documents, Some("id"))
            .await?
            .wait_for_completion(&self.client, None, None)
            .await?
            .to_result()
    }

    async fn delete_index(&self, index: &str) -> anyhow::Result<()> {
        self.client
            .delete_index(index)
            .await?
            .wait_for_completion(&self.client, None, None)
            .await?
            .to_result()
    }
}

trait ToResult {
    fn to_result(self) -> anyhow::Result<()>;
}

impl ToResult for Task {
    fn to_result(self) -> anyhow::Result<()> {
        if self.is_failure() {
            Err(self.unwrap_failure().into())
        } else {
            Ok(())
        }
    }
}
