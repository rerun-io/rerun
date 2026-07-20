use std::ops::ControlFlow;

use anyhow::Context as _;
use url::Url;

use super::ingest::Document;

pub fn connect(url: &str, master_key: &str) -> anyhow::Result<SearchClient> {
    SearchClient::connect(url, master_key)
}

pub struct SearchClient {
    url: String,
    master_key: String,
    agent: ureq::Agent,
}

impl SearchClient {
    /// Connect to Meilisearch.
    ///
    /// `master_key` can be obtained via the Meilisearch could console,
    /// or set via the `--master-key` option when running a local instance.
    pub fn connect(url: &str, master_key: &str) -> anyhow::Result<Self> {
        let this = Self {
            url: url.into(),
            master_key: master_key.into(),
            agent: ureq::Agent::new_with_defaults(),
        };

        this.check_master_key()?;

        Ok(this)
    }

    /// Create an index from `documents`.
    ///
    /// The documents and index settings are first written to a scratch index
    /// (`{index}_build`), which is then atomically swapped with the live index,
    /// so search never observes an empty or partially-built index.
    pub fn index(&self, index: &str, documents: &[Document]) -> anyhow::Result<()> {
        let build_index = format!("{index}_build");

        if self.index_exists(&build_index)? {
            self.delete_index(&build_index)
                .context("failed to delete stale build index")?;
        }
        self.create_index(&build_index)
            .context("failed to create build index")?;
        self.apply_settings(&build_index)
            .context("failed to apply index settings")?;
        self.add_or_replace_documents(&build_index, documents)
            .context("failed to add documents")?;

        // The swap requires both indexes to exist.
        if !self.index_exists(index)? {
            self.create_index(index).context("failed to create index")?;
        }
        self.swap_indexes(index, &build_index)
            .context("failed to swap indexes")?;
        // After the swap the build index holds the previous documents.
        self.delete_index(&build_index)
            .context("failed to delete old index after swap")?;

        println!("created index {index:?}");
        Ok(())
    }

    /// Relevance configuration for the search index.
    ///
    /// Without this, Meilisearch runs on defaults: every field (including `id`
    /// and `url`) is searchable with no priority order, there is no page-type
    /// weighting, and no synonyms — which is how a single API symbol used to
    /// outrank the documentation page on the same topic.
    ///
    /// CANONICAL SOURCE: `scripts/search/search-settings.json` in
    /// rerun-io/landing. The website's `pages` index reads that file directly;
    /// this must stay identical to it (we can't share a file across repos).
    fn settings() -> serde_json::Value {
        serde_json::json!({
            // Order = matching priority: a hit in `title` beats a hit in `content`.
            // `page_title` is display-only: making it searchable let pages
            // whose titles contain query filler ("Migrating from 0.25 to
            // 0.26" matches "to") outrank better results, and page-level
            // findability already comes from each page's intro document.
            "searchableAttributes": ["title", "tags", "hidden_tags", "content"],
            // Docs are indexed one document per `##` section, all sharing
            // their page URL in `page` — so results show at most one (the
            // best-matching) section per page. Other kinds set `page` to
            // their unique URL and are unaffected.
            "distinctAttribute": "page",
            // Default rules plus `weight:desc` (see the `ingest::weight` module) so
            // docs/example pages rank above per-symbol API documents, while
            // `exactness` still lets exact symbol queries win.
            "rankingRules": [
                "words",
                "typo",
                "proximity",
                "attribute",
                "weight:desc",
                "exactness",
            ],
            // Lets the website offer kind tabs/filters and federated queries.
            "filterableAttributes": ["kind", "tags"],
            "sortableAttributes": ["weight"],
            // So natural-language queries like "how do I install" aren't dominated
            // by their filler words. Deliberately omits words that appear inside
            // API identifiers once underscores are tokenized — `to`, `from`,
            // `with`, `as`, `is`, … (`to_arrow`, `log_file_from_path`,
            // `with_sample`, `as_arrow_array`, `is_empty`) — dropping those
            // measurably broke exact symbol queries in the A/B harness.
            "stopWords": [
                "a", "an", "the", "how", "do", "does", "i", "my", "can",
                "what", "when", "where", "which", "you", "your",
            ],
            // Seeded from real queries in PostHog (top clicked + abandoned
            // searches). Keep this list small and principled; one-way synonyms
            // map the user's word to our terminology.
            "synonyms": {
                "headless": ["serve", "server", "no gui"],
                "no gui": ["headless", "serve"],
                "point cloud": ["points3d", "points"],
                "pointcloud": ["point cloud", "points3d"],
                "3d points": ["points3d"], // NOLINT: lowercase user query term
                "splat": ["gaussian splatting"],
                "gaussian splat": ["gaussian splatting", "splat"],
                "ros": ["ros2"],
                "ros2": ["ros"],
                "install": ["installation", "setup"],
                // Marketing-page terms — these pages live in the website's
                // `pages` index, but synonyms are query-side and harmless here;
                // kept so this list matches the canonical settings file.
                "price": ["pricing"],
                "cost": ["pricing"],
                "plans": ["pricing"],
                "job": ["careers"],
                "jobs": ["careers"],
                "hiring": ["careers"],
                "release notes": ["changelog"],
            },
        })
    }

    fn apply_settings(&self, index: &str) -> anyhow::Result<()> {
        let task: Task = self
            .patch(&format!("/indexes/{index}/settings"))
            .send_json(Self::settings())?
            .into_body()
            .read_json()?;
        self.wait_for_task(task)?;
        Ok(())
    }

    fn swap_indexes(&self, a: &str, b: &str) -> anyhow::Result<()> {
        let task: Task = self
            .post("/swap-indexes")
            .send_json(serde_json::json!([{ "indexes": [a, b] }]))?
            .into_body()
            .read_json()?;
        self.wait_for_task(task)?;
        Ok(())
    }

    /// Query a specific index in the database.
    pub fn query(
        &self,
        index: &str,
        q: &str,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<Document>> {
        let Self { url, .. } = self;
        let limit = limit.unwrap_or(20).to_string();
        let url = Url::parse_with_params(
            &format!("{url}/indexes/{index}/search"),
            [("q", q), ("limit", limit.as_str())],
        )?;

        let result: QueryResult<Document> =
            self.get(url.as_str()).call()?.into_body().read_json()?;

        Ok(result.hits)
    }

    fn index_exists(&self, index: &str) -> anyhow::Result<bool> {
        match self.get(&format!("/indexes/{index}")).call() {
            Ok(_) => Ok(true),
            Err(ureq::Error::StatusCode(404)) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn create_index(&self, index: &str) -> anyhow::Result<()> {
        self.post("/indexes")
            .send_json(serde_json::json!({ "uid": index, "primaryKey": Document::PRIMARY_KEY }))?;
        Ok(())
    }

    fn add_or_replace_documents(&self, index: &str, documents: &[Document]) -> anyhow::Result<()> {
        // Meilisearch uses a queue for indexing operations.

        // This call enqueues a task which we have to poll for completion
        let task: Task = self
            .post(&format!("/indexes/{index}/documents"))
            .send_json(documents)?
            .into_body()
            .read_json()?;
        self.wait_for_task(task)?;

        Ok(())
    }

    fn delete_index(&self, index: &str) -> anyhow::Result<()> {
        let task: Task = self
            .delete(&format!("/indexes/{index}"))
            .call()?
            .into_body()
            .read_json()?;
        self.wait_for_task(task).context("while waiting for task")?;
        Ok(())
    }

    fn wait_for_task(&self, mut task: Task) -> anyhow::Result<()> {
        let task_url = format!("/tasks/{}", task.uid);
        loop {
            if task.check_status()?.is_break() {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
            task = self.get(&task_url).call()?.into_body().read_json()?;
        }

        Ok(())
    }

    fn check_master_key(&self) -> anyhow::Result<()> {
        // `/keys` can only be called with a valid master key
        self.get("/keys").call()?;
        Ok(())
    }

    fn get(&self, path: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        let url = format!("{}{path}", self.url);
        self.agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.master_key))
    }

    fn post(&self, path: &str) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
        let url = format!("{}{path}", self.url);
        self.agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.master_key))
    }

    fn patch(&self, path: &str) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
        let url = format!("{}{path}", self.url);
        self.agent
            .patch(&url)
            .header("Authorization", &format!("Bearer {}", self.master_key))
    }

    fn delete(&self, path: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        let url = format!("{}{path}", self.url);
        self.agent
            .delete(&url)
            .header("Authorization", &format!("Bearer {}", self.master_key))
    }
}

#[derive(serde::Deserialize)]
struct QueryResult<T> {
    hits: Vec<T>,
}

#[derive(serde::Deserialize)]
struct Task {
    #[serde(alias = "taskUid")]
    uid: u64,
    status: TaskStatus,
    error: Option<TaskError>,
}

#[derive(serde::Deserialize)]
struct TaskError {
    message: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum TaskStatus {
    Enqueued,
    Processing,
    Succeeded,
    Failed,
    Canceled,
}

impl Task {
    fn check_status(&self) -> anyhow::Result<ControlFlow<()>> {
        match self.status {
            TaskStatus::Enqueued | TaskStatus::Processing => Ok(ControlFlow::Continue(())),

            TaskStatus::Succeeded => Ok(ControlFlow::Break(())),

            TaskStatus::Failed => {
                #[expect(clippy::unwrap_used)]
                let msg = self.error.as_ref().unwrap().message.as_str();
                anyhow::bail!("task failed: {msg}")
            }
            TaskStatus::Canceled => anyhow::bail!("task was canceled"),
        }
    }
}
