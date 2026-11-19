//! [`CatalogTools`]: the catalog the tools act on, and the [`AgentToolbox`] that runs them.

use std::sync::Arc;

use futures::future::BoxFuture;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use strum::VariantArray as _;

use re_agent_tools::{AgentToolbox, ToolDefinition, ToolError, ToolOutput};
use re_async::AsyncRuntimeHandle;
use re_catalog_client::CatalogClient;
use re_redap_client::ConnectionRegistryHandle;
use re_uri::Origin;

use crate::args::ConnectArgs;
use crate::metadata::{
    get_dataset_info, get_dataset_schema, get_segment_properties, list_entries, list_segments,
};
use crate::{CatalogTool, CatalogToolError};

/// What the tools are doing when they connect, for error messages.
const CONNECTING: &str = "connect to the catalog";

/// The catalog the tools act on, and how to reach others.
///
/// Cheap to clone: clones share the chosen catalog. The choice is shared state rather than a
/// parameter because the `connect` tool changes it for every later call, and the toolbox is only
/// ever reached through `&self`, by calls that may run concurrently.
#[derive(Clone)]
pub struct CatalogTools {
    registry: ConnectionRegistryHandle,
    runtime: AsyncRuntimeHandle,
    state: Arc<Mutex<State>>,
}

/// Which catalog the tools act on.
#[derive(Default)]
struct State {
    /// `None` until a catalog is chosen.
    catalog: Option<CatalogClient>,

    /// Whether the agent chose the catalog with the `connect` tool, which
    /// [`CatalogTools::connect_default`] then leaves alone.
    chosen_by_agent: bool,

    /// Counts `connect` calls, so that only the one issued last switches catalogs, even when an
    /// earlier one answers later.
    latest_connect: u64,
}

impl CatalogTools {
    /// Tools that act on the catalog at `catalog`, until the agent connects to another.
    ///
    /// With `None`, the tools act on no catalog until [`Self::connect_default`] or the `connect`
    /// tool picks one. Nothing is dialed until a tool runs.
    pub fn new(
        registry: ConnectionRegistryHandle,
        runtime: AsyncRuntimeHandle,
        catalog: Option<Origin>,
    ) -> Self {
        let catalog = catalog
            .map(|origin| CatalogClient::new(registry.connection_handle(origin), runtime.clone()));
        Self {
            registry,
            runtime,
            state: Arc::new(Mutex::new(State {
                catalog,
                ..Default::default()
            })),
        }
    }

    /// Every catalog tool, as the toolbox offers them, without needing an instance.
    pub fn definitions() -> Vec<ToolDefinition> {
        CatalogTool::VARIANTS
            .iter()
            .copied()
            .map(CatalogTool::definition)
            .collect()
    }

    /// Parse a catalog URL, also accepting plain `http(s)://` for `rerun+http(s)://`.
    pub fn parse_url(url: &str) -> Result<Origin, CatalogToolError> {
        let trimmed = url.trim();
        let rerun_url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            format!("rerun+{trimmed}")
        } else {
            trimmed.to_owned()
        };
        rerun_url
            .parse::<Origin>()
            .map_err(|source| CatalogToolError::InvalidUrl {
                url: url.to_owned(),
                source,
            })
    }

    /// The catalog the tools act on, if any.
    pub fn origin(&self) -> Option<Origin> {
        self.state
            .lock()
            .catalog
            .as_ref()
            .map(|catalog| catalog.origin().clone())
    }

    /// Connect to the catalog at `origin`, unless the agent chose a catalog with the `connect` tool.
    ///
    /// For a host whose default catalog can change, e.g. the catalog of whichever viewer is
    /// connected. Checked and set under one lock, so a concurrent `connect` call always wins.
    pub fn connect_default(&self, origin: Origin) {
        let mut state = self.state.lock();
        let already_there = state
            .catalog
            .as_ref()
            .is_some_and(|catalog| *catalog.origin() == origin);
        if !state.chosen_by_agent && !already_there {
            state.catalog = Some(self.client_for(origin));
        }
    }

    fn client_for(&self, origin: Origin) -> CatalogClient {
        CatalogClient::new(
            self.registry.connection_handle(origin),
            self.runtime.clone(),
        )
    }

    /// Run `tool` with JSON `arguments`, returning its JSON result.
    pub async fn run(
        &self,
        tool: CatalogTool,
        arguments: Map<String, Value>,
    ) -> Result<Value, CatalogToolError> {
        match tool {
            CatalogTool::Connect => {
                let ConnectArgs { url } = parse_args(arguments)?;
                self.connect(&url).await
            }
            CatalogTool::ListEntries => {
                list_entries(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::GetDatasetInfo => {
                get_dataset_info(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::GetDatasetSchema => {
                get_dataset_schema(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::ListSegments => {
                list_segments(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::GetSegmentProperties => {
                get_segment_properties(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::QueryDataset => {
                crate::query::query_dataset(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::QueryTables => {
                crate::query::query_tables(&self.catalog()?, parse_args(arguments)?).await
            }
        }
    }

    fn catalog(&self) -> Result<CatalogClient, CatalogToolError> {
        self.state
            .lock()
            .catalog
            .clone()
            .ok_or(CatalogToolError::NotConnected)
    }

    /// The `connect` tool: switches catalogs only once the new one answers, and only if no
    /// later `connect` call was issued in the meantime.
    async fn connect(&self, url: &str) -> Result<Value, CatalogToolError> {
        let origin = Self::parse_url(url)?;
        let this_connect = {
            let mut state = self.state.lock();
            state.latest_connect += 1;
            state.latest_connect
        };
        let catalog = self.client_for(origin.clone());
        let version = catalog
            .client()
            .await
            .map_err(|err| CatalogToolError::catalog(CONNECTING, err))?
            .version_info()
            .await
            .map_err(|err| CatalogToolError::catalog(CONNECTING, err))?;
        let mut state = self.state.lock();
        if state.latest_connect != this_connect {
            return Err(CatalogToolError::ConnectSuperseded {
                url: url.to_owned(),
            });
        }
        state.catalog = Some(catalog);
        state.chosen_by_agent = true;
        drop(state);
        Ok(json!({
            "connected": origin.to_string(),
            "server_version": version.version,
        }))
    }
}

impl AgentToolbox for CatalogTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        Self::definitions()
    }

    fn requires_approval(&self, name: &str, _arguments: &Map<String, Value>) -> bool {
        CatalogTool::from_name(name).is_none_or(CatalogTool::requires_approval)
    }

    fn call(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> BoxFuture<'static, Result<Vec<ToolOutput>, ToolError>> {
        let Some(tool) = CatalogTool::from_name(name) else {
            let err: ToolError = CatalogToolError::UnknownTool {
                name: name.to_owned(),
            }
            .into();
            return Box::pin(std::future::ready(Err(err)));
        };
        let tools = self.clone();
        let run = async move {
            tools
                .run(tool, arguments)
                .await
                .map(|json| vec![ToolOutput::Text(json.to_string())])
        };
        cfg_select! {
            target_arch = "wasm32" => {
                // The catalog client's futures are not `Send` in the browser.
                let run = re_async::spawn_local_with_result(run);
                Box::pin(async move {
                    match run.await {
                        Ok(result) => result.map_err(ToolError::from),
                        Err(err) => Err(ToolError::from(err)),
                    }
                })
            }
            _ => Box::pin(async move { run.await.map_err(ToolError::from) }),
        }
    }
}

fn parse_args<Args: DeserializeOwned>(
    arguments: Map<String, Value>,
) -> Result<Args, CatalogToolError> {
    serde_json::from_value(Value::Object(arguments))
        .map_err(|source| CatalogToolError::InvalidArguments { source })
}
