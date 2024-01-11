use meilisearch_sdk::{Client, Error, ErrorCode, MeilisearchError, Task};

use crate::ingest::Document;

const INDEX_KEY: &str = "temp";

pub async fn index(documents: &[Document]) -> anyhow::Result<SearchClient> {
    let client = connect();
    if index_exists(&client).await? {
        delete_index(&client).await?; // delete existing index
    }
    create_index(&client, documents).await?;
    Ok(SearchClient { client })
}

pub struct SearchClient {
    client: Client,
}

impl SearchClient {
    pub async fn query(&self, q: &str) -> anyhow::Result<impl Iterator<Item = Document>> {
        let results = self
            .client
            .index(INDEX_KEY)
            .search()
            .with_query(q)
            .execute()
            .await?;
        Ok(results.hits.into_iter().map(|hit| hit.result))
    }
}

fn connect() -> Client {
    Client::new("http://localhost:7700", Some("test"))
}

async fn index_exists(client: &Client) -> anyhow::Result<bool> {
    match client.get_index(INDEX_KEY).await {
        Ok(_) => Ok(true),
        Err(Error::Meilisearch(MeilisearchError {
            error_code: ErrorCode::IndexNotFound,
            ..
        })) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

async fn create_index(client: &Client, documents: &[Document]) -> anyhow::Result<()> {
    client
        .index(INDEX_KEY)
        .add_or_update(documents, Some("id"))
        .await?
        .wait_for_completion(client, None, None)
        .await?
        .to_result()
}

async fn delete_index(client: &Client) -> anyhow::Result<()> {
    client
        .delete_index(INDEX_KEY)
        .await?
        .wait_for_completion(client, None, None)
        .await?
        .to_result()
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
