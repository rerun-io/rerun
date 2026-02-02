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
            agent: ureq::agent(),
        };

        this.check_master_key()?;

        Ok(this)
    }

    /// Create an index from `documents`.
    ///
    /// If an index with the same name already exists, it is deleted first.
    pub fn index(&self, index: &str, documents: &[Document]) -> anyhow::Result<()> {
        if self.index_exists(index)? {
            self.delete_index(index).context("failed to delete index")?; // delete existing index
        }
        self.create_index(index).context("failed to create index")?;
        self.add_or_replace_documents(index, documents)
            .context("failed to add documents")?;
        println!("created index {index:?}");
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

        let result: QueryResult<Document> = self
            .request_with_url(Method::Get, &url)
            .call()?
            .into_json()?;

        Ok(result.hits)
    }

    fn index_exists(&self, index: &str) -> anyhow::Result<bool> {
        match self.get(&format!("/indexes/{index}")).call() {
            Ok(_) => Ok(true),
            Err(ureq::Error::Status(404, _)) => Ok(false),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }

    fn create_index(&self, index: &str) -> anyhow::Result<()> {
        self.post("/indexes")
            .send_json(ureq::json!({ "uid": index, "primaryKey": Document::PRIMARY_KEY }))?;
        Ok(())
    }

    fn add_or_replace_documents(&self, index: &str, documents: &[Document]) -> anyhow::Result<()> {
        // Meilisearch uses a queue for indexing operations.

        // This call enqueues a task which we have to poll for completion
        let task: Task = self
            .post(&format!("/indexes/{index}/documents"))
            .send_json(documents)?
            .into_json()?;
        self.wait_for_task(task)?;

        Ok(())
    }

    fn delete_index(&self, index: &str) -> anyhow::Result<()> {
        let task: Task = self
            .delete(&format!("/indexes/{index}"))
            .call()?
            .into_json()?;
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
            task = self.get(&task_url).call()?.into_json()?;
        }

        Ok(())
    }

    fn check_master_key(&self) -> anyhow::Result<()> {
        // `/keys` can only be called with a valid master key
        self.get("/keys").call()?;
        Ok(())
    }

    /// GET `{self.url}{path}`
    fn get(&self, path: &str) -> ureq::Request {
        self.request(Method::Get, path)
    }

    /// POST `{self.url}{path}`
    fn post(&self, path: &str) -> ureq::Request {
        self.request(Method::Post, path)
    }

    /// DELETE `{self.url}{path}`
    fn delete(&self, path: &str) -> ureq::Request {
        self.request(Method::Delete, path)
    }

    fn request(&self, method: Method, path: &str) -> ureq::Request {
        let Self {
            url, master_key, ..
        } = self;

        self.agent
            .request(method.as_str(), &format!("{url}{path}"))
            .set("Authorization", &format!("Bearer {master_key}"))
    }

    fn request_with_url(&self, method: Method, url: &Url) -> ureq::Request {
        let Self { master_key, .. } = self;

        self.agent
            .request_url(method.as_str(), url)
            .set("Authorization", &format!("Bearer {master_key}"))
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

#[derive(Clone, Copy)]
enum Method {
    Get,
    Post,
    Delete,
}

impl Method {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }
}
