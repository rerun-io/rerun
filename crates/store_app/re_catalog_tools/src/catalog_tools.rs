//! [`CatalogTools`]: the catalog the tools act on, and the [`AgentToolbox`] that runs them.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::future::BoxFuture;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use re_agent_tools::{AgentToolbox, CatalogAccess, ToolDefinition, ToolError, ToolOutput};
use re_async::AsyncRuntimeHandle;
use re_catalog_client::CatalogClient;
#[cfg(not(target_arch = "wasm32"))]
use re_redap_client::KnownServers;
use re_redap_client::{
    ApiError, ApiErrorKind, ClientCredentialsError, ConnectionRegistryHandle, Credentials,
};
use re_uri::Origin;

use crate::args::{ConnectArgs, ListServersArgs, LoginArgs};
#[cfg(not(target_arch = "wasm32"))]
use crate::login::Login;
use crate::metadata::{
    get_dataset_info, get_dataset_schema, get_segment_properties, list_entries, list_segments,
};
use crate::{CatalogTool, CatalogToolError};

/// What the tools are doing when they connect, for error messages.
const CONNECTING: &str = "connect to the catalog";

/// Called with the origin of a catalog that a write tool may have changed.
type OnCatalogChange = Arc<dyn Fn(&Origin) + Send + Sync>;

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

    /// Whether the write tools are offered and run. Shared by clones, so a host can flip it
    /// while a conversation is running.
    write_access: Arc<AtomicBool>,

    /// See [`Self::on_catalog_change`]. Shared by clones.
    on_catalog_change: Arc<Mutex<Option<OnCatalogChange>>>,

    #[cfg(not(target_arch = "wasm32"))]
    login: Arc<Login>,

    /// The catalog servers the user added, which `list_servers` lists. `None` until
    /// [`Self::with_known_servers`] sets it.
    #[cfg(not(target_arch = "wasm32"))]
    known_servers: Option<KnownServers>,
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
            #[cfg(not(target_arch = "wasm32"))]
            login: Arc::new(Login::new(runtime.clone())),
            runtime,
            state: Arc::new(Mutex::new(State {
                catalog,
                ..Default::default()
            })),
            write_access: Arc::new(AtomicBool::new(true)),
            on_catalog_change: Arc::default(),
            #[cfg(not(target_arch = "wasm32"))]
            known_servers: None,
        }
    }

    /// Whether the tools that change the catalog are offered and run. Read-write by default.
    pub fn access(&self) -> CatalogAccess {
        if self.write_access.load(Ordering::Relaxed) {
            CatalogAccess::ReadWrite
        } else {
            CatalogAccess::ReadOnly
        }
    }

    /// Offer and run the tools that change the catalog or not, for this and all clones.
    pub fn set_access(&self, access: CatalogAccess) {
        self.write_access
            .store(access.allows_writes(), Ordering::Relaxed);
    }

    /// Let `list_servers` list the servers in `known_servers`, or none.
    ///
    /// The host decides which list, if any, so that e.g. a viewer that does not persist its
    /// state keeps the agent off the user's real list too.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_known_servers(mut self, known_servers: Option<KnownServers>) -> Self {
        self.known_servers = known_servers;
        self
    }

    /// The `list_servers` tool.
    #[cfg_attr(target_arch = "wasm32", expect(clippy::unnecessary_wraps))]
    fn list_servers(&self) -> Result<Value, CatalogToolError> {
        let servers: Vec<Origin> = cfg_select! {
            target_arch = "wasm32" => Vec::new(),
            _ => match &self.known_servers {
                Some(known) => known
                    .load()
                    .map_err(|source| CatalogToolError::KnownServers { source })?,
                None => Vec::new(),
            },
        };
        Ok(json!({
            "current": self.origin().map(|origin| origin.to_string()),
            "servers": servers.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }))
    }

    /// Every catalog tool, as the toolbox offers them, without needing an instance.
    pub fn definitions() -> Vec<ToolDefinition> {
        CatalogTool::available()
            .map(CatalogTool::definition)
            .collect()
    }

    /// Parse a catalog URL, see [`Origin::from_user_input`].
    pub fn parse_url(url: &str) -> Result<Origin, CatalogToolError> {
        Origin::from_user_input(url).map_err(|source| CatalogToolError::InvalidUrl {
            url: url.to_owned(),
            source,
        })
    }

    // TODO(RR-4859, RR-4853): remove once the catalog server sends an event for every change.
    /// Calls `on_change` after every write tool call, with the origin of the catalog it acted on,
    /// so that a host can show the change. Also after a failed call, which may have changed the
    /// catalog partway.
    ///
    /// Replaces the previous callback, for this and all clones.
    pub fn on_catalog_change(&self, on_change: impl Fn(&Origin) + Send + Sync + 'static) {
        *self.on_catalog_change.lock() = Some(Arc::new(on_change));
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
        if tool.writes() && !self.access().allows_writes() {
            return Err(CatalogToolError::WriteAccessOff);
        }
        let changed_origin = tool.writes().then(|| self.origin()).flatten();
        let result = self.run_unchecked(tool, arguments).await;
        if let Some(origin) = changed_origin {
            let on_change = self.on_catalog_change.lock().clone();
            if let Some(on_change) = on_change {
                on_change(&origin);
            }
        }
        result
    }

    async fn run_unchecked(
        &self,
        tool: CatalogTool,
        arguments: Map<String, Value>,
    ) -> Result<Value, CatalogToolError> {
        match tool {
            CatalogTool::Connect => {
                let ConnectArgs { url } = parse_args(arguments)?;
                self.connect(&url).await
            }
            CatalogTool::ListServers => {
                let ListServersArgs {} = parse_args(arguments)?;
                self.list_servers()
            }
            CatalogTool::Login => {
                let LoginArgs {} = parse_args(arguments)?;
                cfg_select! {
                    target_arch = "wasm32" => Err(CatalogToolError::UnknownTool {
                        name: tool.name().to_owned(),
                    }),
                    _ => self.login.run().await,
                }
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
            CatalogTool::CreateDatasets => {
                crate::write::create_datasets(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::RenameEntries => {
                crate::write::rename_entries(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::UploadRecordings => cfg_select! {
                target_arch = "wasm32" => Err(CatalogToolError::UnknownTool {
                    name: tool.name().to_owned(),
                }),
                _ => crate::write::upload_recordings(&self.catalog()?, parse_args(arguments)?).await,
            },
            CatalogTool::RegisterRecordings => {
                crate::write::register_recordings(&self.catalog()?, parse_args(arguments)?).await
            }
            CatalogTool::RegisterTableBlueprint => {
                crate::write::register_table_blueprint(&self.catalog()?, parse_args(arguments)?)
                    .await
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
        let used_stored_login = self.try_stored_login(&origin);
        let credentials = self.registry.credentials(&origin);
        let catalog = self.client_for(origin.clone());
        let result = Self::catalog_version(&catalog).await;
        if result.is_err() && used_stored_login {
            self.registry.remove_credentials(&origin);
        }
        let version = result.map_err(|err| {
            let can_log_in = login_can_help(
                &err,
                self.registry.should_use_stored_credentials(),
                credentials.as_ref(),
                is_logged_in,
            );
            if can_log_in {
                CatalogToolError::LoginRequired {
                    source: Box::new(err),
                }
            } else {
                CatalogToolError::catalog(CONNECTING, err)
            }
        })?;
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
            "server_version": version,
        }))
    }
}

impl CatalogTools {
    /// Try the login the user saved on this machine for `origin`, unless it has credentials.
    ///
    /// Returns whether it did, so that a failed connect can undo it.
    fn try_stored_login(&self, origin: &Origin) -> bool {
        let uses_stored = self.registry.should_use_stored_credentials();
        if uses_stored && self.registry.credentials(origin).is_none() {
            self.registry.set_credentials(origin, Credentials::Stored);
            true
        } else {
            false
        }
    }

    async fn catalog_version(catalog: &CatalogClient) -> Result<String, ApiError> {
        Ok(catalog.client().await?.version_info().await?.version)
    }
}

/// Whether logging in on this machine can fix the failed connect `err`.
///
/// Only where the connections use the saved login (`uses_stored_credentials`). `credentials` are
/// those the connect used. A login cannot replace an explicit token, nor fix a login the server
/// rejects although the user is logged in (`is_logged_in`).
fn login_can_help(
    err: &ApiError,
    uses_stored_credentials: bool,
    credentials: Option<&Credentials>,
    is_logged_in: impl FnOnce() -> bool,
) -> bool {
    if err.kind != ApiErrorKind::Unauthenticated
        || !uses_stored_credentials
        || matches!(credentials, Some(Credentials::Token(_)))
    {
        return false;
    }
    let source = err
        .source
        .as_deref()
        .and_then(|source| source.downcast_ref::<ClientCredentialsError>());
    match source {
        Some(
            ClientCredentialsError::UnauthenticatedMissingToken(_)
            | ClientCredentialsError::NotAuthorized
            | ClientCredentialsError::SessionExpired
            | ClientCredentialsError::RefreshError(_),
        ) => true,
        Some(ClientCredentialsError::UnauthenticatedBadToken { .. }) => !is_logged_in(),
        Some(ClientCredentialsError::HostMismatch(_)) | None => false,
    }
}

/// Whether the user has a login saved on this machine, or in this browser.
fn is_logged_in() -> bool {
    match re_auth::oauth::load_credentials() {
        Ok(credentials) => credentials.is_some(),
        Err(err) => {
            re_log::debug!("Failed to load the saved login: {err}");
            false
        }
    }
}

impl AgentToolbox for CatalogTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        CatalogTool::available()
            .filter(|tool| !tool.writes() || self.access().allows_writes())
            .map(CatalogTool::definition)
            .collect()
    }

    fn requires_approval(&self, name: &str, _arguments: &Map<String, Value>) -> bool {
        match CatalogTool::from_name(name) {
            None => true,
            // Refused without asking, since the user turned writes off.
            Some(tool) if tool.writes() && !self.access().allows_writes() => false,
            Some(tool) => tool.requires_approval(),
        }
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use re_redap_client::{CredentialSource, SourcedCredentials};

    use super::*;

    fn unauthenticated(source: ClientCredentialsError) -> ApiError {
        ApiError::credentials_with_source(&Origin::http_local_host(1), None, source, "connect")
    }

    fn rejected_token() -> ClientCredentialsError {
        ClientCredentialsError::UnauthenticatedBadToken {
            status: tonic::Status::unauthenticated("bad token").into(),
            credentials: SourcedCredentials {
                source: CredentialSource::PerOrigin,
                credentials: Credentials::Stored,
            },
        }
    }

    #[test]
    fn login_helps_without_a_login() {
        let err = unauthenticated(ClientCredentialsError::NotAuthorized);
        assert!(login_can_help(
            &err,
            true,
            Some(&Credentials::Stored),
            || false
        ));
    }

    #[test]
    fn login_helps_when_no_login_is_saved() {
        assert!(login_can_help(
            &unauthenticated(rejected_token()),
            true,
            Some(&Credentials::Stored),
            || false
        ));
    }

    #[test]
    fn login_does_not_help_when_the_server_rejects_the_saved_login() {
        assert!(!login_can_help(
            &unauthenticated(rejected_token()),
            true,
            Some(&Credentials::Stored),
            || true
        ));
    }

    #[test]
    fn login_does_not_help_a_token_for_another_host() {
        let err = unauthenticated(ClientCredentialsError::HostMismatch(
            re_auth::HostMismatchError {
                host: "example.com".to_owned(),
            },
        ));
        assert!(!login_can_help(
            &err,
            true,
            Some(&Credentials::Stored),
            || false
        ));
    }

    #[test]
    fn login_does_not_replace_an_explicit_token() {
        let err = unauthenticated(ClientCredentialsError::NotAuthorized);
        let jwt = re_auth::Jwt::try_from(
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.c2lnbmF0dXJl".to_owned(),
        )
        .expect("a JWT");
        let token = Credentials::Token(jwt);
        assert!(!login_can_help(&err, true, Some(&token), || false));
    }

    #[test]
    fn login_helps_an_expired_session() {
        let err = unauthenticated(ClientCredentialsError::SessionExpired);
        assert!(login_can_help(
            &err,
            true,
            Some(&Credentials::Stored),
            || true
        ));
    }

    #[test]
    fn login_helps_when_the_login_cannot_be_refreshed() {
        let err = unauthenticated(ClientCredentialsError::RefreshError(
            tonic::Status::unauthenticated("refresh token expired").into(),
        ));
        assert!(login_can_help(
            &err,
            true,
            Some(&Credentials::Stored),
            || true
        ));
    }

    #[test]
    fn login_helps_when_no_token_was_sent() {
        let err = unauthenticated(ClientCredentialsError::UnauthenticatedMissingToken(
            tonic::Status::unauthenticated("missing token").into(),
        ));
        assert!(login_can_help(&err, true, None, || false));
    }

    #[test]
    fn login_does_not_help_without_stored_credentials() {
        let err = unauthenticated(ClientCredentialsError::NotAuthorized);
        assert!(!login_can_help(&err, false, None, || false));
    }

    #[test]
    fn login_does_not_help_other_failures() {
        let err = ApiError::connection(&Origin::http_local_host(1), "connection refused");
        assert!(!login_can_help(&err, true, None, || false));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lists_the_servers_the_user_added() {
        let dir = tempfile::tempdir().expect("temp dir");
        let known = KnownServers::at(dir.path().join("servers.json"));
        let added = Origin::http_local_host(51234);
        known.add(&added).expect("add");

        let runtime =
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("runtime");
        let current = Origin::http_local_host(1);
        let tools = CatalogTools::new(
            re_redap_client::ConnectionRegistry::new_without_stored_credentials(),
            runtime,
            Some(current.clone()),
        )
        .with_known_servers(Some(known));
        let servers = tools
            .run(CatalogTool::ListServers, Map::new())
            .await
            .expect("servers");
        assert_eq!(
            servers,
            json!({
                "current": current.to_string(),
                "servers": [added.to_string()],
            })
        );
    }
}
