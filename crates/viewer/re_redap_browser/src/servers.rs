use std::collections::BTreeMap;
use std::sync::Arc;
use std::task::Poll;

use datafusion::prelude::{SessionConfig, SessionContext, col, lit};
use egui::{Frame, Margin, RichText};
use re_dataframe_ui::{ColumnBlueprint, default_display_name_for_column};
use re_log_types::{EntityPathPart, EntryId};
use re_protos::cloud::v1alpha1::{EntryKind, ScanSegmentTableResponse};
use re_quota_channel::send_crossbeam;
use re_redap_client::{
    ClientCredentialsError, ConnectionRegistryHandle, CredentialSource, Credentials,
};
use re_sorbet::ColumnDescriptorRef;
use re_ui::alert::Alert;
use re_ui::{UiExt as _, icons};
use re_viewer_context::{
    AppContext, AsyncRuntimeHandle, EditRedapServerModalCommand, ViewerContext,
};

use crate::context::Context;
use crate::entries::{Dataset, Entries, Entry, Table};
use crate::server_modal::{LoginFlow, LoginFlowResult, ServerModal, ServerModalMode};

pub struct Server {
    origin: re_uri::Origin,
    entries: Entries,

    /// Session context wrapper which holds all the table-like entries of the server.
    tables_session_ctx: Arc<SessionContext>,

    connection_registry: re_redap_client::ConnectionRegistryHandle,
    runtime: AsyncRuntimeHandle,
}

impl Server {
    fn new(
        connection_registry: re_redap_client::ConnectionRegistryHandle,
        runtime: AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) -> Self {
        let tables_session_ctx = Self::session_context();

        let entries = Entries::new(
            connection_registry.clone(),
            &runtime,
            egui_ctx,
            origin.clone(),
            tables_session_ctx.clone(),
        );

        Self {
            origin,
            entries,
            tables_session_ctx,
            connection_registry,
            runtime,
        }
    }

    fn session_context() -> Arc<SessionContext> {
        let session_ctx = SessionContext::new_with_config(
            SessionConfig::new()
                // In order to quickly show results when filtering a table, we disable batch coalescing.
                // This may be slightly inefficient, but is worth it if the user sees immediate
                // results.
                .with_coalesce_batches(false),
        );
        Arc::new(session_ctx)
    }

    fn refresh_entries(&mut self, runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context) {
        // Note: this also drops the DataFusionTableWidget caches
        self.tables_session_ctx = Self::session_context();

        self.entries = Entries::new(
            self.connection_registry.clone(),
            runtime,
            egui_ctx,
            self.origin.clone(),
            self.tables_session_ctx.clone(),
        );
    }

    #[inline]
    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    #[inline]
    pub fn entries(&self) -> &Entries {
        &self.entries
    }

    fn on_frame_start(&mut self) {
        self.entries.on_frame_start();
    }

    fn find_entry(&self, entry_id: EntryId) -> Option<&Entry> {
        self.entries.find_entry(entry_id)
    }

    fn title_ui(
        &self,
        title: String,
        ctx: &Context<'_>,
        ui: &mut egui::Ui,
        content: impl FnOnce(&mut egui::Ui),
    ) {
        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new(title).strong());
                if ui
                    .small_icon_button(&icons::RESET, "Refresh collection")
                    .clicked()
                {
                    send_crossbeam(
                        ctx.command_sender,
                        Command::RefreshCollection(self.origin.clone()),
                    )
                    .ok();
                }
            });

            ui.add_space(12.0);

            content(ui);
        });
    }

    /// Central panel UI for when a server is selected.
    fn server_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ctx: &Context<'_>,
        ui: &mut egui::Ui,
        has_active_login_flow: bool,
    ) {
        if let Poll::Ready(Err(err)) = self.entries.state() {
            self.title_ui(self.origin.host.to_string(), ctx, ui, |ui| {
                error_ui(
                    viewer_ctx,
                    ctx,
                    ui,
                    &self.origin,
                    err,
                    has_active_login_flow,
                );
            });
            return;
        }

        const ENTRY_LINK_COLUMN_NAME: &str = "link";

        re_dataframe_ui::DataFusionTableWidget::new(self.tables_session_ctx.clone(), "__entries")
            .title(self.origin.host.to_string())
            .column_blueprint(|desc| {
                let mut blueprint = ColumnBlueprint::default();

                if let ColumnDescriptorRef::Component(component) = desc
                    && component.component == "entry_kind"
                {
                    blueprint = blueprint.variant_ui(re_component_ui::REDAP_ENTRY_KIND_VARIANT);
                }

                let column_sort_key = match desc.display_name().as_str() {
                    "name" => 0,
                    ENTRY_LINK_COLUMN_NAME => 1,
                    _ => 2,
                };

                blueprint = blueprint.sort_key(column_sort_key);

                if desc.display_name().as_str() == ENTRY_LINK_COLUMN_NAME {
                    blueprint = blueprint.variant_ui(re_component_ui::REDAP_URI_BUTTON_VARIANT);
                }

                blueprint
            })
            .generate_entry_links(ENTRY_LINK_COLUMN_NAME, "id", self.origin.clone())
            .prefilter(
                col("entry_kind")
                    .in_list(
                        vec![lit(EntryKind::Table as i32), lit(EntryKind::Dataset as i32)],
                        false,
                    )
                    .and(col("name").not_eq(lit("__entries"))),
            )
            .show(viewer_ctx, &self.runtime, ui);
    }

    fn dataset_entry_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        dataset: &Dataset,
    ) {
        const RECORDING_LINK_COLUMN_NAME: &str = "recording link";

        re_dataframe_ui::DataFusionTableWidget::new(
            self.tables_session_ctx.clone(),
            dataset.name(),
        )
        .title(dataset.name())
        .url(re_uri::EntryUri::new(dataset.origin.clone(), dataset.id()).to_string())
        .column_blueprint(|desc| {
            let mut name = default_display_name_for_column(desc);

            // strip prefix and remove underscores, _only_ for the base columns (aka not the
            // properties)
            name = name
                .strip_prefix("rerun_")
                .map(|name| name.replace('_', " "))
                .unwrap_or(name);

            let default_visible = if desc.entity_path().is_some_and(|entity_path| {
                entity_path.starts_with(&std::iter::once(EntityPathPart::properties()).collect())
            }) {
                // Property columns are visible by default
                true
            } else {
                matches!(
                    desc.display_name().as_str(),
                    RECORDING_LINK_COLUMN_NAME | ScanSegmentTableResponse::FIELD_SEGMENT_ID
                )
            };

            let column_sort_key = match desc.display_name().as_str() {
                ScanSegmentTableResponse::FIELD_SEGMENT_ID => 0,
                RECORDING_LINK_COLUMN_NAME => 1,
                _ => 2,
            };

            let mut blueprint = ColumnBlueprint::default()
                .display_name(name)
                .default_visibility(default_visible)
                .sort_key(column_sort_key);

            if desc.display_name().as_str() == RECORDING_LINK_COLUMN_NAME {
                blueprint = blueprint.variant_ui(re_component_ui::REDAP_URI_BUTTON_VARIANT);
            }

            blueprint
        })
        .generate_segment_links(
            RECORDING_LINK_COLUMN_NAME,
            ScanSegmentTableResponse::FIELD_SEGMENT_ID,
            self.origin.clone(),
            dataset.id(),
        )
        .show(viewer_ctx, &self.runtime, ui);
    }

    fn table_entry_ui(&self, viewer_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, table: &Table) {
        re_dataframe_ui::DataFusionTableWidget::new(self.tables_session_ctx.clone(), table.name())
            .title(table.name())
            .url(re_uri::EntryUri::new(table.origin.clone(), table.id()).to_string())
            .show(viewer_ctx, &self.runtime, ui);
    }
}

fn error_ui(
    viewer_ctx: &ViewerContext<'_>,
    ctx: &Context<'_>,
    ui: &mut egui::Ui,
    origin: &re_uri::Origin,
    err: &re_redap_client::ApiError,
    has_active_login_flow: bool,
) {
    if let Some(conn_err) = err.as_client_credentials_error() {
        let message = match conn_err {
            ClientCredentialsError::RefreshError { .. } => {
                "There was an error refreshing your credentials"
            }

            ClientCredentialsError::SessionExpired => "Your session has expired",

            ClientCredentialsError::UnauthenticatedMissingToken { .. } => {
                "This server requires authentication to access its data."
            }

            ClientCredentialsError::UnauthenticatedBadToken { credentials, .. } => {
                match credentials.source {
                    CredentialSource::PerOrigin => "The credentials for this origin are invalid",
                    CredentialSource::Fallback => "The fallback credentials are invalid",
                    CredentialSource::EnvVar => {
                        "The credentials provided via environment variable REDAP_TOKEN are invalid"
                    }
                }
            }

            ClientCredentialsError::HostMismatch(_) => "The token is not allowed for this server",
        };

        let show_login = match conn_err {
            ClientCredentialsError::RefreshError(_)
            | ClientCredentialsError::SessionExpired
            | ClientCredentialsError::UnauthenticatedMissingToken(_)
            | ClientCredentialsError::UnauthenticatedBadToken { .. } => true,
            ClientCredentialsError::HostMismatch(_) => false,
        };

        if show_login {
            Alert::info().show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.strong(message);

                    ui.add_space(8.0);
                    if has_active_login_flow {
                        ui.horizontal(|ui| {
                            ui.loading_indicator("Waiting for login");
                            ui.label("Waiting for login…");
                        });
                    } else {
                        ui.horizontal(|ui| {
                            if let Some(auth) = viewer_ctx.app_ctx.auth_context {
                                // User is already logged in — offer to use stored credentials
                                if ui
                                    .add(
                                        re_ui::ReButton::new(format!("Continue as {}", auth.email))
                                            .primary()
                                            .small(),
                                    )
                                    .clicked()
                                {
                                    send_crossbeam(
                                        ctx.command_sender,
                                        Command::UseStoredCredentials(origin.clone()),
                                    )
                                    .ok();
                                }
                            } else {
                                // User is not logged in — start login flow
                                if ui
                                    .add(re_ui::ReButton::new("Log in").primary().small())
                                    .clicked()
                                {
                                    send_crossbeam(
                                        ctx.command_sender,
                                        Command::StartLoginFlow(origin.clone()),
                                    )
                                    .ok();
                                }
                            }
                            if ui
                                .add(re_ui::ReButton::new("Edit connection").small())
                                .clicked()
                            {
                                send_crossbeam(
                                    ctx.command_sender,
                                    Command::OpenEditServerModal(EditRedapServerModalCommand {
                                        origin: origin.clone(),
                                        open_on_success: None,
                                        title: None,
                                    }),
                                )
                                .ok();
                            }
                        });
                    }
                });
            });
        } else {
            warning_with_edit_button(ctx, ui, origin, message);
        }
    } else if matches!(
        &err.kind,
        re_redap_client::ApiErrorKind::InvalidServer | re_redap_client::ApiErrorKind::Connection
    ) {
        warning_with_edit_button(ctx, ui, origin, &err.to_string());
    } else {
        ui.error_label(err.to_string());
    }
}

fn warning_with_edit_button(
    ctx: &Context<'_>,
    ui: &mut egui::Ui,
    origin: &re_uri::Origin,
    message: &str,
) {
    Alert::warning().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.strong(message);
            ui.add_space(8.0);
            if ui
                .add(re_ui::ReButton::new("Edit connection").small())
                .clicked()
            {
                send_crossbeam(
                    ctx.command_sender,
                    Command::OpenEditServerModal(EditRedapServerModalCommand {
                        origin: origin.clone(),
                        open_on_success: None,
                        title: None,
                    }),
                )
                .ok();
            }
        });
    });
}

/// All servers known to the viewer, and their catalog data.
pub struct RedapServers {
    servers: BTreeMap<re_uri::Origin, Server>,

    /// When deserializing we can't construct the [`Server`]s right away
    /// so they get queued here.
    pending_servers: Vec<re_uri::Origin>,

    // message queue for commands
    command_sender: crossbeam::channel::Sender<Command>,
    command_receiver: crossbeam::channel::Receiver<Command>,

    server_modal_ui: ServerModal,

    /// Active inline login flow with the origin it was started for.
    ///
    /// That origin will get the token and be refreshed on login.
    inline_login_flow: Option<(re_uri::Origin, Box<LoginFlow>)>,
}

impl serde::Serialize for RedapServers {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.servers
            .keys()
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for RedapServers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let origins = Vec::<re_uri::Origin>::deserialize(deserializer)?;

        let mut servers = Self::default();

        // We cannot create `Server` right away, because we need an async handle and an
        // `egui::Context` for that, so we just queue commands to be processed early next frame.
        for origin in origins {
            servers.pending_servers.push(origin);
        }

        Ok(servers)
    }
}

impl Default for RedapServers {
    fn default() -> Self {
        let (command_sender, command_receiver) = create_channel(256);

        Self {
            servers: Default::default(),
            pending_servers: Default::default(),
            command_sender,
            command_receiver,
            server_modal_ui: Default::default(),
            inline_login_flow: None,
        }
    }
}

/// Create a blocking channel on native, and an unbounded channel on web.
fn create_channel<T>(
    size: usize,
) -> (
    crossbeam::channel::Sender<T>,
    crossbeam::channel::Receiver<T>,
) {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            _ = size;
            crossbeam::channel::unbounded() // we're not allowed to block on web
        } else {
            crossbeam::channel::bounded(size)
        }
    }
}

pub enum Command {
    /// Open a modal to add a new server.
    OpenAddServerModal,

    /// Open a modal to edit an existing server.
    OpenEditServerModal(EditRedapServerModalCommand),

    /// Add a server with an optional JWT token.
    ///
    /// If the token is None, this does *not* remove an existing token.
    ///
    /// The closure can be used to run something after adding the server (useful since [`Command`]s
    /// are not ran in order with [`re_viewer_context::SystemCommand`]s).
    AddServer {
        origin: re_uri::Origin,
        credentials: Option<re_redap_client::Credentials>,
        on_add: Option<Box<dyn FnOnce() + Send>>,
    },

    /// Remove a server and its token.
    RemoveServer(re_uri::Origin),

    RefreshCollection(re_uri::Origin),

    /// Start an inline login flow for a server.
    StartLoginFlow(re_uri::Origin),

    /// Use the stored account credentials for a server and refresh.
    UseStoredCredentials(re_uri::Origin),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenAddServerModal => write!(f, "OpenAddServerModal"),
            Self::OpenEditServerModal(cmd) => {
                f.debug_tuple("OpenEditServerModal").field(cmd).finish()
            }
            Self::AddServer {
                origin,
                credentials,
                on_add,
            } => f
                .debug_struct("AddServer")
                .field("origin", origin)
                .field("credentials", credentials)
                .field("on_add", &on_add.as_ref().map(|_| "…"))
                .finish(),
            Self::RemoveServer(origin) => f.debug_tuple("RemoveServer").field(origin).finish(),
            Self::RefreshCollection(origin) => {
                f.debug_tuple("RefreshCollection").field(origin).finish()
            }
            Self::StartLoginFlow(origin) => f.debug_tuple("StartLoginFlow").field(origin).finish(),
            Self::UseStoredCredentials(origin) => {
                f.debug_tuple("UseStoredCredentials").field(origin).finish()
            }
        }
    }
}

impl RedapServers {
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty() && self.pending_servers.is_empty()
    }

    /// Whether we already know about a given server (or have it queued to be added).
    pub fn has_server(&self, origin: &re_uri::Origin) -> bool {
        self.servers.contains_key(origin) || self.pending_servers.contains(origin)
    }

    /// Add a server to the hub.
    pub fn add_server(&self, origin: re_uri::Origin) {
        send_crossbeam(
            &self.command_sender,
            Command::AddServer {
                origin,
                credentials: None,
                on_add: None,
            },
        )
        .ok();
    }

    pub fn iter_servers(&self) -> impl Iterator<Item = &Server> {
        self.servers.values()
    }

    pub fn is_authenticated(&self, origin: &re_uri::Origin) -> bool {
        self.servers
            .get(origin)
            .and_then(|server| server.connection_registry.credentials(origin))
            .is_some()
    }

    pub fn logout(&mut self) -> Vec<re_uri::Origin> {
        self.inline_login_flow = None;
        self.server_modal_ui.logout();
        // Log out from the servers that used the accounts token.
        let mut origins = Vec::new();
        for server in self.servers.values() {
            if matches!(
                server.connection_registry.credentials(&server.origin),
                Some(Credentials::Stored)
            ) {
                origins.push(server.origin.clone());
                server
                    .connection_registry
                    .remove_credentials(&server.origin);
                send_crossbeam(
                    &self.command_sender,
                    Command::RefreshCollection(server.origin.clone()),
                )
                .ok();
            }
        }
        origins
    }

    /// Per-frame housekeeping.
    ///
    /// - Process commands from the queue.
    /// - Update all servers.
    pub fn on_frame_start(
        &mut self,
        connection_registry: &ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
    ) {
        self.pending_servers.drain(..).for_each(|origin| {
            send_crossbeam(
                &self.command_sender,
                Command::AddServer {
                    origin,
                    credentials: None,
                    on_add: None,
                },
            )
            .ok();
        });
        while let Ok(command) = self.command_receiver.try_recv() {
            self.handle_command(connection_registry, runtime, egui_ctx, command);
        }

        // Poll inline login flow
        if let Some((origin, flow)) = &mut self.inline_login_flow
            && let Some(result) = flow.poll()
        {
            let origin = origin.clone();
            match result {
                LoginFlowResult::Success => {
                    send_crossbeam(&self.command_sender, Command::UseStoredCredentials(origin))
                        .ok();
                }
                LoginFlowResult::Failure(err) => {
                    re_log::warn!("Login failed: {err}");
                }
            }
            self.inline_login_flow = None;
        }

        for server in self.servers.values_mut() {
            server.on_frame_start();
        }
    }

    fn handle_command(
        &mut self,
        connection_registry: &re_redap_client::ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        command: Command,
    ) {
        match command {
            Command::OpenAddServerModal => {
                self.server_modal_ui
                    .open(ServerModalMode::Add, connection_registry);
            }

            Command::OpenEditServerModal(origin) => {
                self.server_modal_ui
                    .open(ServerModalMode::Edit(origin), connection_registry);
            }

            Command::AddServer {
                origin,
                credentials,
                on_add,
            } => {
                if let Some(credentials) = credentials {
                    connection_registry.set_credentials(&origin, credentials);
                }
                if !self.servers.contains_key(&origin) {
                    self.servers.insert(
                        origin.clone(),
                        Server::new(
                            connection_registry.clone(),
                            runtime.clone(),
                            egui_ctx,
                            origin.clone(),
                        ),
                    );
                } else {
                    // Since we persist the server list on disk this happens quite often.
                    // E.g. run `pixi run rerun "rerun+http://localhost"` more than once.
                    re_log::debug!(
                        "Tried to add pre-existing server at {:?}",
                        origin.to_string()
                    );
                }
                if let Some(on_add) = on_add {
                    on_add();
                }
            }

            Command::RemoveServer(origin) => {
                self.servers.remove(&origin);
                connection_registry.remove_credentials(&origin);
            }

            Command::RefreshCollection(origin) => {
                self.servers.entry(origin).and_modify(|server| {
                    server.refresh_entries(runtime, egui_ctx);
                });
            }

            Command::StartLoginFlow(origin) => {
                if self.inline_login_flow.is_none() {
                    match LoginFlow::open_and_start(egui_ctx) {
                        Ok(flow) => {
                            self.inline_login_flow = Some((origin, Box::new(flow)));
                        }
                        Err(err) => {
                            re_log::error!("Failed to start login: {err}");
                        }
                    }
                }
            }

            Command::UseStoredCredentials(origin) => {
                connection_registry.set_credentials(&origin, re_redap_client::Credentials::Stored);
                send_crossbeam(&self.command_sender, Command::RefreshCollection(origin)).ok();
            }
        }
    }

    pub fn server_central_panel_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        origin: &re_uri::Origin,
    ) {
        if let Some(server) = self.servers.get(origin) {
            let has_login_flow = self.has_active_login_flow(origin);
            self.with_ctx(|ctx| {
                server.server_ui(viewer_ctx, ctx, ui, has_login_flow);
            });
        } else {
            viewer_ctx.revert_to_default_display_mode();
        }
    }

    pub fn open_add_server_modal(&self) {
        send_crossbeam(&self.command_sender, Command::OpenAddServerModal).ok();
    }

    pub fn open_edit_server_modal(&self, command: EditRedapServerModalCommand) {
        send_crossbeam(&self.command_sender, Command::OpenEditServerModal(command)).ok();
    }

    pub fn entry_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        active_entry: EntryId,
    ) {
        for server in self.servers.values() {
            if let Some(entry) = server.find_entry(active_entry) {
                match entry.inner() {
                    Ok(crate::entries::EntryInner::Dataset(dataset)) => {
                        server.dataset_entry_ui(viewer_ctx, ui, dataset);

                        // If we're connected twice to the same server, we will find this entry
                        // multiple times. We avoid it by returning here.
                        return;
                    }
                    Ok(crate::entries::EntryInner::Table(table)) => {
                        server.table_entry_ui(viewer_ctx, ui, table);

                        // If we're connected twice to the same server, we will find this entry
                        // multiple times. We avoid it by returning here.
                        return;
                    }
                    Err(err) => {
                        Frame::new().inner_margin(16.0).show(ui, |ui| {
                            Alert::error().show_text(
                                ui,
                                format!("Error loading entry {}", entry.name()),
                                Some(err.to_string()),
                            );
                        });
                    }
                }
            }
        }
    }

    pub fn modals_ui(&mut self, app_ctx: &AppContext<'_>, ui: &egui::Ui) {
        //TODO(ab): borrow checker doesn't let me use `with_ctx()` here, I should find a better way
        let ctx = Context {
            command_sender: &self.command_sender,
        };

        self.server_modal_ui.ui(app_ctx, &ctx, ui);
    }

    pub fn send_command(&self, command: Command) {
        let result = send_crossbeam(&self.command_sender, command);

        if let Err(err) = result {
            re_log::warn_once!("Failed to send command: {err}");
        }
    }

    fn has_active_login_flow(&self, origin: &re_uri::Origin) -> bool {
        self.inline_login_flow
            .as_ref()
            .is_some_and(|(o, _)| o == origin)
    }

    #[inline]
    fn with_ctx<R>(&self, func: impl FnOnce(&Context<'_>) -> R) -> R {
        let ctx = Context {
            command_sender: &self.command_sender,
        };

        func(&ctx)
    }
}
