//! The backend for an agent harness: a subprocess that speaks ACP over stdio.

use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Agent, Client, ConnectionTo, Error, JsonRpcResponse,
};
use agent_client_protocol_schema::ProtocolVersion;
use agent_client_protocol_schema::v1::{
    AuthMethod, AuthenticateRequest, CancelNotification, ClientCapabilities, ConfigOptionUpdate,
    CreateElicitationRequest, CurrentModeUpdate, ElicitationCapabilities,
    ElicitationFormCapabilities, Implementation, InitializeRequest, InitializeResponse,
    LoadSessionRequest, McpServer, McpServerStdio, NewSessionRequest, PromptRequest,
    RequestPermissionRequest, SessionConfigId, SessionConfigKind, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOptions, SessionConfigValueId, SessionId,
    SessionModeId, SessionModeState, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, SetSessionModeRequest, StopReason,
};
use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use re_async::UiWaker;

use crate::agent_harness::acp_error::describe_error;
use crate::agent_harness::{AgentHarnessConfig, McpStdioServer};
use crate::backend::{AgentCommand, AgentConnection, AgentEvent, EventSender, SessionOrigin};
use crate::{LineDirection, Responder};

/// Spawns an agent subprocess and connects to it over ACP.
///
/// All protocol work happens on a dedicated thread. `waker` is woken after every event so the UI
/// can repaint.
pub fn spawn(config: AgentHarnessConfig, waker: UiWaker) -> AgentConnection {
    let (connection, command_rx, events) = AgentConnection::new(waker);
    let spawned = std::thread::Builder::new()
        .name("re_agent_ui-acp".to_owned())
        .spawn(move || run(config, command_rx, &events));
    if let Err(err) = spawned {
        re_log::error!("Failed to spawn agent thread: {err}");
    }
    connection
}

/// Reports the completed turn.
///
/// If authentication failed, requests login _after_ reporting turn completion so that
/// completion cannot overwrite the waiting-for-login state.
fn finish_turn(
    events: &EventSender,
    result: Result<StopReason, Error>,
    auth_methods: Vec<AuthMethod>,
) {
    let auth_error = result
        .as_ref()
        .err()
        .filter(|err| err.code == Error::auth_required().code)
        .map(describe_error);
    // Finish the turn before entering the login state: finishing resets the session phase.
    events.send(AgentEvent::TurnFinished(
        result.map_err(|err| describe_error(&err)),
    ));
    if let Some(message) = auth_error {
        events.send(AgentEvent::AuthRequired {
            methods: auth_methods,
            message,
        });
    }
}

/// Hands out a [`Responder`] whose answer the returned task forwards to the agent.
///
/// The task must run on the ACP connection. If the agent cancels the request first, the task
/// drops the receiver, which [`Responder::is_cancelled`] then reports.
/// The task never fails, since a failed task shuts the whole connection down.
pub fn forward_answer<T: JsonRpcResponse + Send + 'static>(
    acp_responder: agent_client_protocol::Responder<T>,
) -> (
    Responder<T>,
    impl Future<Output = Result<(), Error>> + Send + 'static,
) {
    let (responder, answer) = Responder::channel();
    let task = async move {
        let cancellation = acp_responder.cancellation();
        let cancelled = std::pin::pin!(cancellation.cancelled());
        // An answer that never comes, or comes after cancellation, leaves the request unanswered.
        if let futures::future::Either::Left((Ok(response), _)) =
            futures::future::select(answer, cancelled).await
            && let Err(err) = acp_responder.respond(response)
        {
            re_log::debug!("Failed to answer the agent: {}", describe_error(&err));
        }
        Ok(())
    };
    (responder, task)
}

fn run(
    config: AgentHarnessConfig,
    command_rx: UnboundedReceiver<AgentCommand>,
    events: &EventSender,
) {
    let agent_config = AcpAgentConfig::new(config.command.clone())
        .args(config.args.iter().cloned())
        .envs(config.env.iter().cloned());

    let log_protocol = config.log_protocol;
    let log_events = events.clone();
    let agent = AcpAgent::new(agent_config).with_debug(move |line, direction| {
        let direction = match direction {
            agent_client_protocol::LineDirection::Stdin => LineDirection::Stdin,
            agent_client_protocol::LineDirection::Stdout => LineDirection::Stdout,
            agent_client_protocol::LineDirection::Stderr => LineDirection::Stderr,
        };
        if log_protocol || direction == LineDirection::Stderr {
            log_events.send(AgentEvent::Log {
                direction,
                line: line.to_owned(),
            });
        }
    });

    let notification_events = events.clone();
    let permission_events = events.clone();
    let elicitation_events = events.clone();
    let foreground_events = events.clone();

    let result = futures::executor::block_on(
        Client
            .builder()
            .name("re_agent_ui")
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    notification_events.send(AgentEvent::Update(notification.update));
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: RequestPermissionRequest, acp_responder, cx| {
                    let (responder, task) = forward_answer(acp_responder);
                    permission_events.send(AgentEvent::PermissionRequest { request, responder });
                    cx.spawn(task)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: CreateElicitationRequest, acp_responder, cx| {
                    let (responder, task) = forward_answer(acp_responder);
                    elicitation_events.send(AgentEvent::ElicitationRequest { request, responder });
                    cx.spawn(task)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, |cx: ConnectionTo<Agent>| async move {
                drive(cx, config, command_rx, foreground_events).await
            }),
    );

    if let Err(err) = result {
        events.send(AgentEvent::Error(format!(
            "Agent connection failed: {}",
            describe_error(&err)
        )));
    }
    events.send(AgentEvent::Disconnected);
}

/// The foreground of the connection: initialize, open a session, then serve UI commands.
async fn drive(
    cx: ConnectionTo<Agent>,
    config: AgentHarnessConfig,
    mut command_rx: UnboundedReceiver<AgentCommand>,
    events: EventSender,
) -> Result<(), Error> {
    events.send(AgentEvent::Status("Initializing…".to_owned()));

    let client_info = Implementation::new("re_agent_ui", env!("CARGO_PKG_VERSION"));
    let init = cx
        .send_request(
            InitializeRequest::new(ProtocolVersion::V1)
                .client_info(client_info)
                .client_capabilities(ClientCapabilities::new().elicitation(
                    ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()),
                )),
        )
        .block_task()
        .await?;

    events.send(AgentEvent::Initialized {
        agent_info: init.agent_info.clone(),
        capabilities: Box::new(init.agent_capabilities.clone()),
        auth_methods: init.auth_methods.clone(),
    });

    let mut session_id = start_session(
        &cx,
        &config,
        &events,
        &init,
        config.session_to_load.as_ref(),
    )
    .await;

    while let Some(command) = command_rx.next().await {
        match command {
            AgentCommand::Prompt(prompt) => {
                let Some(session_id) = &session_id else {
                    events.send(AgentEvent::Error("No active session".to_owned()));
                    continue;
                };
                let events = events.clone();
                let auth_methods = init.auth_methods.clone();
                cx.send_request(PromptRequest::new(session_id.clone(), prompt))
                    .on_receiving_result(move |result| async move {
                        finish_turn(
                            &events,
                            result.map(|response| response.stop_reason),
                            auth_methods,
                        );
                        Ok(())
                    })?;
            }

            AgentCommand::Cancel => {
                if let Some(session_id) = &session_id {
                    cx.send_notification(CancelNotification::new(session_id.clone()))?;
                }
            }

            AgentCommand::SetMode(mode_id) => {
                let Some(session_id) = &session_id else {
                    continue;
                };
                let events = events.clone();
                let request = SetSessionModeRequest::new(session_id.clone(), mode_id.clone());
                cx.send_request(request)
                    .on_receiving_result(move |result| async move {
                        match result {
                            // Agents need not announce the mode they were just told to use,
                            // so report it ourselves once they have accepted it.
                            Ok(_) => events.send(AgentEvent::Update(
                                SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(mode_id)),
                            )),
                            Err(err) => events.send(AgentEvent::Error(format!(
                                "Failed to set mode: {}",
                                describe_error(&err)
                            ))),
                        }
                        Ok(())
                    })?;
            }

            AgentCommand::SetConfigOption { config_id, value } => {
                let Some(session_id) = &session_id else {
                    continue;
                };
                let events = events.clone();
                let request =
                    SetSessionConfigOptionRequest::new(session_id.clone(), config_id, value);
                cx.send_request(request)
                    .on_receiving_result(move |result| async move {
                        match result {
                            Ok(response) => {
                                events.send(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
                                    ConfigOptionUpdate::new(response.config_options),
                                )));
                            }
                            Err(err) => events.send(AgentEvent::Error(format!(
                                "Failed to change the session configuration: {}",
                                describe_error(&err)
                            ))),
                        }
                        Ok(())
                    })?;
            }

            AgentCommand::Authenticate(method_id) => {
                events.send(AgentEvent::Status("Authenticating…".to_owned()));
                match cx
                    .send_request(AuthenticateRequest::new(method_id))
                    .block_task()
                    .await
                {
                    Ok(_) => {
                        // Continue the conversation the login interrupted, if there was one.
                        let session_to_load = session_id.or_else(|| config.session_to_load.clone());
                        session_id =
                            start_session(&cx, &config, &events, &init, session_to_load.as_ref())
                                .await;
                    }
                    Err(err) => {
                        events.send(AgentEvent::Error(format!(
                            "Authentication failed: {}",
                            describe_error(&err)
                        )));
                    }
                }
            }

            AgentCommand::Shutdown => break,
        }
    }

    Ok(())
}

/// A session the agent has opened, before the preferred model and mode are applied.
struct OpenedSession {
    session_id: SessionId,
    modes: Option<SessionModeState>,
    config_options: Option<Vec<SessionConfigOption>>,

    origin: SessionOrigin,
}

/// Loads `session_to_load` if the agent can, and otherwise opens a new session.
async fn start_session(
    cx: &ConnectionTo<Agent>,
    config: &AgentHarnessConfig,
    events: &EventSender,
    init: &InitializeResponse,
    session_to_load: Option<&SessionId>,
) -> Option<SessionId> {
    let mcp_servers: Vec<McpServer> = config
        .mcp_servers
        .iter()
        .map(|server| {
            let McpStdioServer {
                name,
                command,
                args,
            } = server.clone();
            McpServer::Stdio(McpServerStdio::new(name, command).args(args))
        })
        .collect();

    if session_to_load.is_some() {
        // Also when the agent cannot load sessions, so that the user is told the conversation
        // starts over.
        events.send(AgentEvent::LoadingSession);
    }
    let loadable = session_to_load.filter(|_| init.agent_capabilities.load_session);
    let opened = match loadable {
        Some(session_id) => {
            let request = LoadSessionRequest::new(session_id.clone(), config.cwd.clone())
                .additional_directories(config.additional_directories.clone())
                .mcp_servers(mcp_servers.clone());
            match cx.send_request(request).block_task().await {
                Ok(response) => Ok(OpenedSession {
                    session_id: session_id.clone(),
                    modes: response.modes,
                    config_options: response.config_options,
                    origin: SessionOrigin::Loaded,
                }),
                Err(err) if err.code == Error::auth_required().code => Err(err),
                Err(err) => {
                    re_log::warn!(
                        "Failed to load the earlier agent session, so starting a new one: {}",
                        describe_error(&err)
                    );
                    new_session(cx, config, events, mcp_servers).await
                }
            }
        }
        None => new_session(cx, config, events, mcp_servers).await,
    };

    match opened {
        Ok(mut opened) => {
            if let Some(config_options) = select_model(
                cx,
                &opened.session_id,
                opened.config_options.as_deref().unwrap_or_default(),
                config.preferred_model.as_ref(),
                &config.model_preferences,
            )
            .await
            {
                opened.config_options = Some(config_options);
            }
            if let Some((config_id, speed)) = pick_speed(
                opened.config_options.as_deref().unwrap_or_default(),
                config.preferred_speed.as_ref(),
            ) {
                let request =
                    SetSessionConfigOptionRequest::new(opened.session_id.clone(), config_id, speed);
                match cx.send_request(request).block_task().await {
                    Ok(updated) => opened.config_options = Some(updated.config_options),
                    Err(err) => re_log::debug!("Failed to select speed: {}", describe_error(&err)),
                }
            }
            if let Some(config_options) = select_effort(
                cx,
                &opened.session_id,
                opened.config_options.as_deref().unwrap_or_default(),
                config.preferred_effort.as_ref(),
            )
            .await
            {
                opened.config_options = Some(config_options);
            }
            if let Some(selected_mode) = select_mode(
                cx,
                &opened.session_id,
                opened.modes.as_ref(),
                &config.mode_preferences,
            )
            .await
                && let Some(modes) = &mut opened.modes
            {
                modes.current_mode_id = selected_mode;
            }
            let OpenedSession {
                session_id,
                modes,
                config_options,
                origin,
            } = opened;
            events.send(AgentEvent::SessionStarted {
                session_id: session_id.clone(),
                modes,
                config_options: config_options.unwrap_or_default(),
                origin,
            });
            Some(session_id)
        }
        Err(err) if err.code == Error::auth_required().code => {
            events.send(AgentEvent::AuthRequired {
                methods: init.auth_methods.clone(),
                message: describe_error(&err),
            });
            None
        }
        Err(err) => {
            events.send(AgentEvent::Error(format!(
                "Failed to start session: {}",
                describe_error(&err)
            )));
            None
        }
    }
}

async fn new_session(
    cx: &ConnectionTo<Agent>,
    config: &AgentHarnessConfig,
    events: &EventSender,
    mcp_servers: Vec<McpServer>,
) -> Result<OpenedSession, Error> {
    events.send(AgentEvent::Status("Starting session…".to_owned()));
    let request = NewSessionRequest::new(config.cwd.clone())
        .additional_directories(config.additional_directories.clone())
        .mcp_servers(mcp_servers);
    let response = cx.send_request(request).block_task().await?;
    Ok(OpenedSession {
        session_id: response.session_id,
        modes: response.modes,
        config_options: response.config_options,
        origin: SessionOrigin::New,
    })
}

/// Selects the preferred mode before the session can receive its first prompt.
async fn select_mode(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    modes: Option<&SessionModeState>,
    preferences: &[SessionModeId],
) -> Option<SessionModeId> {
    let mode = pick_mode(modes, preferences)?;
    let request = SetSessionModeRequest::new(session_id.clone(), mode.clone());
    match cx.send_request(request).block_task().await {
        Ok(_) => Some(mode),
        Err(err) => {
            re_log::debug!("Failed to select session mode: {}", describe_error(&err));
            None
        }
    }
}

/// Returns the first preferred mode the agent offers, when it differs from the agent's default.
fn pick_mode(
    modes: Option<&SessionModeState>,
    preferences: &[SessionModeId],
) -> Option<SessionModeId> {
    let modes = modes?;
    let preference = preferences.iter().find(|preference| {
        modes
            .available_modes
            .iter()
            .any(|mode| &mode.id == *preference)
    })?;
    (&modes.current_mode_id != preference).then(|| preference.clone())
}

/// Restores an offered saved model, otherwise selects the first offered model preference.
///
/// Awaited before the session is announced, so that the first prompt already runs on the chosen
/// model. A failure is not fatal: the session then runs on the agent's own default model.
async fn select_model(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    config_options: &[SessionConfigOption],
    preferred_model: Option<&SessionConfigValueId>,
    preferences: &[String],
) -> Option<Vec<SessionConfigOption>> {
    let (config_id, model) = pick_model(config_options, preferred_model, preferences)?;
    let request = SetSessionConfigOptionRequest::new(session_id.clone(), config_id, model);
    match cx.send_request(request).block_task().await {
        Ok(response) => Some(response.config_options),
        Err(err) => {
            re_log::debug!("Failed to select a model: {}", describe_error(&err));
            None
        }
    }
}

/// The speed to restore at startup, as a `session/set_config_option` request.
///
/// `None` when the agent does not offer the preferred speed, or already uses it.
fn pick_speed(
    config_options: &[SessionConfigOption],
    preferred_speed: Option<&SessionConfigValueId>,
) -> Option<(SessionConfigId, SessionConfigValueId)> {
    let speed = preferred_speed?;
    let (config_id, select) = crate::transcript::speed_selector(config_options)?;
    let offered = crate::transcript::select_options(select)
        .iter()
        .any(|option| option.value == *speed);
    (select.current_value != *speed && offered).then(|| (config_id.clone(), speed.clone()))
}

/// The model selector and the model to set it to, or `None` when there is nothing to do.
///
/// `None` also when the preferred model is already the current one, so that an agent that has
/// no `session/set_config_option` support is never asked for one.
fn pick_model(
    config_options: &[SessionConfigOption],
    preferred_model: Option<&SessionConfigValueId>,
    preferences: &[String],
) -> Option<(SessionConfigId, SessionConfigValueId)> {
    if preferred_model.is_none() && preferences.is_empty() {
        return None;
    }

    let selector = config_options
        .iter()
        .find(|option| option.category == Some(SessionConfigOptionCategory::Model))?;
    let SessionConfigKind::Select(select) = &selector.kind else {
        return None;
    };
    let models = crate::transcript::select_options(select);

    let model = preferred_model
        .and_then(|preferred| {
            models
                .iter()
                .copied()
                .find(|model| &model.value == preferred)
        })
        .or_else(|| crate::transcript::preferred_model(&models, preferences))?;

    (model.value != select.current_value).then(|| (selector.id.clone(), model.value.clone()))
}

/// Restores a saved effort if the agent offers it.
///
/// Runs after [`select_model`], because the levels an agent offers depend on the model.
/// A failure is not fatal: the session then runs on the agent's own default level.
async fn select_effort(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    config_options: &[SessionConfigOption],
    preferred_effort: Option<&crate::Effort>,
) -> Option<Vec<SessionConfigOption>> {
    let (config_id, level) = pick_effort(config_options, preferred_effort?)?;
    let request = SetSessionConfigOptionRequest::new(session_id.clone(), config_id, level);
    match cx.send_request(request).block_task().await {
        Ok(response) => Some(response.config_options),
        Err(err) => {
            re_log::debug!("Failed to select an effort: {}", describe_error(&err));
            None
        }
    }
}

/// The effort selector and the level to set it to, or `None` when the agent does not
/// offer `preferred_effort` or already uses it.
fn pick_effort(
    config_options: &[SessionConfigOption],
    preferred_effort: &crate::Effort,
) -> Option<(SessionConfigId, SessionConfigValueId)> {
    let preferred_level = &SessionConfigValueId::from(preferred_effort);
    let selector = config_options
        .iter()
        .find(|option| option.category == Some(SessionConfigOptionCategory::ThoughtLevel))?;
    let SessionConfigKind::Select(select) = &selector.kind else {
        return None;
    };
    let offered = match &select.options {
        SessionConfigSelectOptions::Ungrouped(levels) => {
            levels.iter().any(|level| &level.value == preferred_level)
        }
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| &group.options)
            .any(|level| &level.value == preferred_level),
        _ => false,
    };

    (offered && &select.current_value != preferred_level)
        .then(|| (selector.id.clone(), preferred_level.clone()))
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::Channel;
    use agent_client_protocol_schema::v1::{
        NewSessionResponse, SessionConfigSelectGroup, SessionConfigSelectOption, SessionMode,
        SetSessionConfigOptionResponse,
    };

    use super::*;

    /// Checks that authentication failures request login _after_ finishing the turn,
    /// while successful turns finish without requesting login.
    #[test]
    fn prompt_auth_failure_finishes_turn_before_requesting_login() {
        let (events, rx) = EventSender::new(UiWaker::noop());

        // Authentication failure must finish the turn before requesting login.
        finish_turn(&events, Err(Error::auth_required()), Vec::new());

        assert!(matches!(
            rx.try_recv().unwrap(),
            AgentEvent::TurnFinished(Err(_))
        ));
        assert!(matches!(
            rx.try_recv().unwrap(),
            AgentEvent::AuthRequired { .. }
        ));
        assert!(rx.try_recv().is_err());

        // A successful turn must not request login.
        finish_turn(&events, Ok(StopReason::EndTurn), Vec::new());
        assert!(matches!(
            rx.try_recv().unwrap(),
            AgentEvent::TurnFinished(Ok(_))
        ));
        assert!(rx.try_recv().is_err());
    }

    fn claude_models(current: &str) -> Vec<SessionConfigOption> {
        let models = vec![
            SessionConfigSelectOption::new("default", "Default"),
            SessionConfigSelectOption::new("opus", "Opus 4.6"),
            SessionConfigSelectOption::new("sonnet", "Sonnet 4.5"),
            SessionConfigSelectOption::new("haiku", "Haiku 4.5"),
        ];
        vec![
            SessionConfigOption::boolean("brave_mode", "Brave Mode", false),
            SessionConfigOption::select("model", "Model", current.to_owned(), models)
                .category(SessionConfigOptionCategory::Model),
        ]
    }

    fn preferences(models: &[&str]) -> Vec<String> {
        models.iter().map(|model| (*model).to_owned()).collect()
    }

    #[test]
    fn picks_the_first_preferred_mode_that_is_offered() {
        let modes = SessionModeState::new(
            "manual",
            vec![
                SessionMode::new("manual", "Manual"),
                SessionMode::new("plan", "Plan"),
                SessionMode::new("auto", "Auto"),
            ],
        );
        let pick = |preferences: &[&str]| {
            let preferences: Vec<_> = preferences
                .iter()
                .map(|id| SessionModeId::new(*id))
                .collect();
            pick_mode(Some(&modes), &preferences).map(|mode| mode.0.to_string())
        };

        assert_eq!(pick(&["plan", "auto"]).as_deref(), Some("plan"));
        assert_eq!(pick(&["yolo", "auto"]).as_deref(), Some("auto"));

        // Nothing to change when the agent already is in the preferred mode.
        assert_eq!(pick(&["manual", "auto"]), None);

        assert_eq!(pick(&[]), None);
        assert_eq!(pick(&["yolo"]), None);
        assert_eq!(pick_mode(None, &[SessionModeId::new("plan")]), None);
    }

    fn claude_effort(current: &str) -> Vec<SessionConfigOption> {
        let levels = vec![
            SessionConfigSelectOption::new("default", "Default"),
            SessionConfigSelectOption::new("low", "Low"),
            SessionConfigSelectOption::new("high", "High"),
        ];
        vec![
            SessionConfigOption::select("effort", "Effort", current.to_owned(), levels)
                .category(SessionConfigOptionCategory::ThoughtLevel),
        ]
    }

    #[test]
    fn restores_an_offered_effort() {
        let (config_id, level) =
            pick_effort(&claude_effort("default"), &crate::Effort("high".to_owned()))
                .expect("the effort selector");
        assert_eq!(config_id.0.as_ref(), "effort");
        assert_eq!(level.0.as_ref(), "high");
    }

    #[test]
    fn leaves_the_effort_when_the_saved_one_does_not_apply() {
        let options = claude_effort("high");
        assert_eq!(
            pick_effort(&options, &crate::Effort("high".to_owned())),
            None
        );
        assert_eq!(
            pick_effort(&options, &crate::Effort("max".to_owned())),
            None
        );
        assert_eq!(
            pick_effort(&claude_models("opus"), &crate::Effort("high".to_owned())),
            None
        );
    }

    #[test]
    fn picks_the_first_preferred_model_that_is_offered() {
        let picked = pick_model(
            &claude_models("opus"),
            None,
            &preferences(&["flash", "sonnet"]),
        );
        let (config_id, model) = picked.expect("the model selector");
        assert_eq!(config_id.0.as_ref(), "model");
        assert_eq!(model.0.as_ref(), "sonnet");
    }

    #[test]
    fn matches_a_preference_inside_a_model_name() {
        let models = vec![
            SessionConfigOption::select(
                "model",
                "Model",
                "gemini-2.5-pro",
                vec![
                    SessionConfigSelectOption::new("gemini-2.5-pro", "Gemini 2.5 Pro"),
                    SessionConfigSelectOption::new("gemini-2.5-flash", "Gemini 2.5 Flash"),
                ],
            )
            .category(SessionConfigOptionCategory::Model),
        ];
        let picked = pick_model(&models, None, &preferences(&["sonnet", "flash"]));
        assert_eq!(picked.expect("a match").1.0.as_ref(), "gemini-2.5-flash");
    }

    #[test]
    fn finds_models_inside_groups() {
        let group = SessionConfigSelectGroup::new(
            "anthropic",
            "Anthropic",
            vec![SessionConfigSelectOption::new("sonnet", "Sonnet 4.5")],
        );
        let models = vec![
            SessionConfigOption::select("model", "Model", "opus", vec![group])
                .category(SessionConfigOptionCategory::Model),
        ];
        assert_eq!(
            pick_model(&models, None, &preferences(&["sonnet"]))
                .expect("a match")
                .1
                .0
                .as_ref(),
            "sonnet"
        );
    }

    #[test]
    fn leaves_the_agent_alone_when_there_is_nothing_to_do() {
        // No preferences, and no model selector at all:
        assert_eq!(pick_model(&claude_models("opus"), None, &[]), None);
        let modes = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "normal",
                vec![SessionConfigSelectOption::new("normal", "Normal")],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];
        assert_eq!(pick_model(&modes, None, &preferences(&["sonnet"])), None);

        // Offered, but not what we asked for, and already what we would ask for:
        assert_eq!(
            pick_model(&claude_models("opus"), None, &preferences(&["flash"])),
            None
        );
        assert_eq!(
            pick_model(&claude_models("sonnet"), None, &preferences(&["sonnet"])),
            None
        );
    }

    /// A saved speed is restored also when the agent groups its speed options.
    #[test]
    fn picks_a_grouped_speed() {
        let group = SessionConfigSelectGroup::new(
            "tiers",
            "Tiers",
            vec![
                SessionConfigSelectOption::new("off", "Off"),
                SessionConfigSelectOption::new("on", "On"),
            ],
        );
        let options = vec![SessionConfigOption::select(
            "fast",
            "Speed",
            "off",
            vec![group],
        )];
        let on = SessionConfigValueId::new("on");
        assert_eq!(
            pick_speed(&options, Some(&on)),
            Some((SessionConfigId::new("fast"), on))
        );
    }

    /// Startup selects the model before restoring a speed that only that model offers.
    #[test]
    fn restores_speed_from_the_selected_models_options() {
        check_startup(
            Some(OfferedSpeed {
                current: "off",
                values: &["off", "on"],
            }),
            SpeedRequest::Accepted,
        );
    }

    /// Missing, unsupported, and already-selected speeds cause no speed request.
    #[test]
    fn skips_speed_requests_that_do_not_apply() {
        check_startup(None, SpeedRequest::Omitted);
        check_startup(
            Some(OfferedSpeed {
                current: "off",
                values: &["off"],
            }),
            SpeedRequest::Omitted,
        );
        check_startup(
            Some(OfferedSpeed {
                current: "on",
                values: &["off", "on"],
            }),
            SpeedRequest::Omitted,
        );
    }

    /// A rejected speed request leaves the session ready with its confirmed settings.
    #[test]
    fn a_speed_request_failure_does_not_abort_startup() {
        check_startup(
            Some(OfferedSpeed {
                current: "off",
                values: &["off", "on"],
            }),
            SpeedRequest::Rejected,
        );
    }

    /// The speed selector offered after selecting the chosen model.
    #[derive(Clone, Copy)]
    struct OfferedSpeed {
        current: &'static str,
        values: &'static [&'static str],
    }

    #[derive(Clone, Copy)]
    enum SpeedRequest {
        Accepted,
        Omitted,
        Rejected,
    }

    fn startup_options(model: &str, speed: Option<OfferedSpeed>) -> Vec<SessionConfigOption> {
        let mut options = vec![
            SessionConfigOption::select(
                "model",
                "Model",
                model.to_owned(),
                vec![
                    SessionConfigSelectOption::new("initial", "Initial"),
                    SessionConfigSelectOption::new("chosen", "Chosen"),
                ],
            )
            .category(SessionConfigOptionCategory::Model),
        ];
        if let Some(OfferedSpeed { current, values }) = speed {
            options.push(SessionConfigOption::select(
                "fast",
                "Speed",
                current.to_owned(),
                values
                    .iter()
                    .map(|&value| SessionConfigSelectOption::new(value.to_owned(), value))
                    .collect::<Vec<_>>(),
            ));
        }
        options
    }

    /// Runs startup against an in-process ACP agent that offers `speed` only after model selection.
    ///
    /// Checks the request order and the confirmed options reported in `SessionStarted`.
    fn check_startup(speed: Option<OfferedSpeed>, speed_request: SpeedRequest) {
        let config = AgentHarnessConfig {
            command: "unused".into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: ".".into(),
            additional_directories: Vec::new(),
            off_limits_directories: Vec::new(),
            mcp_servers: Vec::new(),
            log_protocol: false,
            preamble: None,
            model_preferences: Vec::new(),
            preferred_model: Some(SessionConfigValueId::new("chosen")),
            preferred_speed: Some(SessionConfigValueId::new("on")),
            preferred_effort: None,
            mode_preferences: Vec::new(),
            session_to_load: None,
        };
        let init = InitializeResponse::new(ProtocolVersion::V1);
        let post_model = startup_options("chosen", speed);
        let confirmed = match speed_request {
            SpeedRequest::Accepted => startup_options(
                "chosen",
                speed.map(|speed| OfferedSpeed {
                    current: "on",
                    ..speed
                }),
            ),
            SpeedRequest::Omitted | SpeedRequest::Rejected => post_model.clone(),
        };
        let response_options = confirmed.clone();
        let (request_tx, mut request_rx) = futures::channel::mpsc::unbounded();
        let (client_channel, agent_channel) = Channel::duplex();
        let (done_tx, done_rx) = futures::channel::oneshot::channel();
        let (events, event_rx) = EventSender::new(UiWaker::noop());

        futures::executor::block_on(async {
            let agent = Agent
                .builder()
                .on_receive_request(
                    async |_request: NewSessionRequest, responder, _cx| {
                        responder.respond(
                            NewSessionResponse::new("test-session")
                                .config_options(startup_options("initial", None)),
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: SetSessionConfigOptionRequest, responder, _cx| {
                        request_tx
                            .unbounded_send((request.config_id.clone(), request.value.clone()))
                            .expect("request receiver open");
                        if request.config_id.0.as_ref() == "model" {
                            responder
                                .respond(SetSessionConfigOptionResponse::new(post_model.clone()))
                        } else if matches!(speed_request, SpeedRequest::Rejected) {
                            responder.respond_with_error(Error::internal_error())
                        } else {
                            responder.respond(SetSessionConfigOptionResponse::new(
                                response_options.clone(),
                            ))
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .connect_with(agent_channel, async move |_cx| {
                    // Keep the agent connected until the client has finished startup.
                    done_rx.await.expect("startup completed");
                    Ok::<_, Error>(())
                });
            let client = Client.builder().connect_with(
                client_channel,
                async move |cx: ConnectionTo<Agent>| {
                    let session_id = start_session(&cx, &config, &events, &init, None).await;
                    assert_eq!(session_id.unwrap().0.as_ref(), "test-session");
                    done_tx.send(()).expect("agent waiting");
                    Ok::<_, Error>(())
                },
            );
            let (client, agent) = futures::join!(client, agent);
            client.expect("client connection");
            agent.expect("agent connection");
        });

        let mut expected = vec![("model".into(), "chosen".into())];
        if !matches!(speed_request, SpeedRequest::Omitted) {
            expected.push(("fast".into(), "on".into()));
        }
        let requests: Vec<_> = std::iter::from_fn(|| request_rx.try_recv().ok()).collect();
        assert_eq!(requests, expected);
        let started = event_rx.try_iter().find_map(|event| match event {
            AgentEvent::SessionStarted { config_options, .. } => Some(config_options),
            _ => None,
        });
        assert_eq!(started, Some(confirmed));
    }
}
