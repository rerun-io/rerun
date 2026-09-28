//! The command operations of the `ViewerControlService` API: `ListCommands`, `DescribeCommands`,
//! and `RunCommand`, which expose the command palette.

use re_log_channel::ViewerControlError;
use re_log_types::StoreId;
use re_protos::viewer_control::v1alpha1::{
    CommandSummary, DescribeCommandsRequest, DescribeCommandsResponse, ListCommandsRequest,
    ListCommandsResponse, RunCommandRequest, RunCommandResponse, ViewerCommand,
};
use re_ui::{BoundCommand, CommandEnvironment, CommandKind, CommandScope};
use re_viewer_context::StoreHub;

use super::super::App;

impl App {
    /// The environment the command palette sees, with the caller's `store_id` and
    /// `server_origin` (if any) in place of the active recording and the selected server.
    fn command_environment_for_request(
        &self,
        store_hub: &StoreHub,
        store_id: Option<String>,
        server_origin: Option<String>,
    ) -> Result<CommandEnvironment, ViewerControlError> {
        let mut env = self.command_environment();

        if let Some(store_id) = store_id {
            let store_id = store_id.parse::<StoreId>().map_err(|err| {
                ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
            })?;
            if store_hub.entity_db(&store_id).is_none() {
                return Err(ViewerControlError::not_found(format!(
                    "Recording {store_id} is not open"
                )));
            }
            env.recording = Some(store_id);
        }

        if let Some(origin) = server_origin {
            let origin = origin.parse::<re_uri::Origin>().map_err(|err| {
                ViewerControlError::invalid_argument(format!("invalid server_origin: {err}"))
            })?;
            if !self.state.redap_servers.has_server(&origin) {
                return Err(ViewerControlError::not_found(format!(
                    "The viewer is not connected to the server {origin}"
                )));
            }
            env.has_editable_redap_server = !self.state.redap_servers.is_internal_server(&origin);
            env.redap_server = Some(origin);
        }

        Ok(env)
    }

    pub fn list_commands(
        &self,
        store_hub: &StoreHub,
        request: ListCommandsRequest,
    ) -> Result<ListCommandsResponse, ViewerControlError> {
        let ListCommandsRequest {
            include_descriptions,
            include_unavailable,
            store_id,
            server_origin,
        } = request;
        let include_descriptions = include_descriptions.unwrap_or(false);
        let include_unavailable = include_unavailable.unwrap_or(false);

        let env = self.command_environment_for_request(store_hub, store_id, server_origin)?;
        let mut commands: Vec<CommandSummary> = CommandKind::all()
            .filter(|kind| include_unavailable || kind.bind(&env).is_some())
            .map(|kind| CommandSummary {
                id: kind.id(),
                description: include_descriptions.then(|| kind.tooltip().to_owned()),
            })
            .collect();
        commands.sort_by_key(|command| command.id.clone());
        Ok(ListCommandsResponse { commands })
    }

    pub fn describe_commands(
        &self,
        store_hub: &StoreHub,
        request: DescribeCommandsRequest,
        egui_ctx: &egui::Context,
    ) -> Result<DescribeCommandsResponse, ViewerControlError> {
        let DescribeCommandsRequest {
            ids,
            store_id,
            server_origin,
        } = request;

        let unknown: Vec<String> = ids
            .iter()
            .filter(|id| CommandKind::from_id(id).is_none())
            .map(|id| format!("{id:?}"))
            .collect();
        if !unknown.is_empty() {
            return Err(ViewerControlError::not_found(format!(
                "Unknown command ids: {}. Call `list_commands` for the valid ids.",
                unknown.join(", ")
            )));
        }

        let env = self.command_environment_for_request(store_hub, store_id, server_origin)?;
        Ok(DescribeCommandsResponse {
            commands: ids
                .iter()
                .filter_map(|id| CommandKind::from_id(id))
                .map(|kind| describe_command(kind, &env, egui_ctx))
                .collect(),
        })
    }

    pub fn run_command(
        &self,
        store_hub: &StoreHub,
        request: RunCommandRequest,
    ) -> Result<RunCommandResponse, ViewerControlError> {
        let RunCommandRequest {
            id,
            store_id,
            server_origin,
            allow_blocking_native_modal,
        } = request;

        let kind = CommandKind::from_id(&id).ok_or_else(|| {
            ViewerControlError::not_found(format!(
                "Unknown command id {id:?}. Call `list_commands` for the valid ids."
            ))
        })?;

        if store_id.is_some() && kind.scope() != CommandScope::Recording {
            return Err(ViewerControlError::invalid_argument(format!(
                "`store_id` only applies to `recording.*` commands, not to {id:?}"
            )));
        }
        if server_origin.is_some() && kind.scope() != CommandScope::RedapServer {
            return Err(ViewerControlError::invalid_argument(format!(
                "`server_origin` only applies to `server.*` commands, not to {id:?}"
            )));
        }
        let env = self.command_environment_for_request(store_hub, store_id, server_origin)?;

        if kind.opens_blocking_native_modal() && !allow_blocking_native_modal.unwrap_or(false) {
            return Err(ViewerControlError::failed_precondition(format!(
                "{id:?} opens a native modal, which freezes the viewer until a person answers it. \
                 Set `allow_blocking_native_modal` only if someone is at the viewer to do that."
            )));
        }

        let command = kind.bind(&env).ok_or_else(|| {
            ViewerControlError::failed_precondition(format!(
                "{id:?} cannot run right now: {}",
                unavailable_reason(kind, &env)
            ))
        })?;

        let target = command_target(&command);
        self.command_sender.send_command(command);
        Ok(RunCommandResponse { id, target })
    }
}

fn describe_command(
    kind: CommandKind,
    env: &CommandEnvironment,
    egui_ctx: &egui::Context,
) -> ViewerCommand {
    let bound = kind.bind(env);
    ViewerCommand {
        id: kind.id(),
        title: kind.text().to_owned(),
        description: kind.tooltip().to_owned(),
        scope: match kind.scope() {
            CommandScope::Global => "global",
            CommandScope::Recording => "recording",
            CommandScope::RedapServer => "server",
            CommandScope::Table => "table",
        }
        .to_owned(),
        shortcuts: kind
            .kb_shortcuts(egui_ctx.os())
            .iter()
            .map(|shortcut| egui_ctx.format_shortcut(shortcut))
            .collect(),
        available: bound.is_some(),
        blocking_native_modal: kind.opens_blocking_native_modal(),
        destructive: kind.is_destructive(),
        target: bound.as_ref().and_then(command_target),
    }
}

/// What a bound command acts on, as a caller would name it. `None` for global commands.
fn command_target(command: &BoundCommand) -> Option<String> {
    match command {
        BoundCommand::Ui(_) => None,
        BoundCommand::Recording(cmd) => Some(cmd.recording_id.to_string()),
        BoundCommand::RedapServer(cmd) => Some(cmd.origin.to_string()),
        BoundCommand::Table(cmd) => Some(match &cmd.table {
            re_uri::TableReference::LocalTable(table_id) => table_id.to_string(),
            re_uri::TableReference::RedapServerEntries { origin } => {
                format!("{origin} (entries table)")
            }
            re_uri::TableReference::RedapEntry { origin, entry_id } => {
                re_uri::EntryUri::new(origin.clone(), *entry_id).to_string()
            }
        }),
    }
}

/// Why [`CommandKind::bind`] found nothing for `kind` to act on in `env`.
fn unavailable_reason(kind: CommandKind, env: &CommandEnvironment) -> &'static str {
    match kind.scope() {
        CommandScope::Global => "this build of the viewer does not support it",
        CommandScope::Recording => "no recording is active; open one, or pass `store_id`",
        CommandScope::RedapServer => {
            if env.redap_server.is_none() {
                "no server is selected; pass `server_origin`"
            } else {
                "the viewer's built-in catalog cannot be edited or removed"
            }
        }
        CommandScope::Table => {
            if env.table.is_none() {
                "no dataset or table is being viewed"
            } else {
                "it does not apply to the dataset or table being viewed"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_command_environment() -> CommandEnvironment {
        CommandEnvironment {
            recording: None,
            redap_server: None,
            has_editable_redap_server: false,
            table: None,
        }
    }

    #[test]
    fn a_recording_command_targets_the_active_recording() {
        let kind = CommandKind::from_id("recording.toggle_play_pause").unwrap();
        let egui_ctx = egui::Context::default();
        // Formatting a shortcut needs the fonts, which the first frame loads.
        egui_ctx
            .run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();

        let described = describe_command(kind, &empty_command_environment(), &egui_ctx);
        assert!(!described.available);
        assert_eq!(described.target, None);
        assert_eq!(described.scope, "recording");
        assert_eq!(described.shortcuts, ["Space"]);

        let recording = StoreId::random(re_log_types::StoreKind::Recording, "test_app");
        let env = CommandEnvironment {
            recording: Some(recording.clone()),
            ..empty_command_environment()
        };
        let described = describe_command(kind, &env, &egui_ctx);
        assert!(described.available);
        assert_eq!(described.target, Some(recording.to_string()));
    }
}
