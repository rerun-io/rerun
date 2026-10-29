//! The backend that runs the agent loop itself, calling a model provider's API
//! directly through `rig-core`.

use std::sync::Arc;

use agent_client_protocol_schema::v1::{
    AgentCapabilities, ConfigOptionUpdate, ContentBlock, ContentChunk, ImageContent,
    Implementation, PermissionOption, PermissionOptionKind, PromptCapabilities,
    RequestPermissionOutcome, RequestPermissionRequest, SessionConfigId, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOption, SessionConfigValueId, SessionId,
    SessionUpdate, StopReason, TextContent, ToolCall as AcpToolCall, ToolCallId, ToolCallStatus,
    ToolCallUpdate, ToolCallUpdateFields, ToolKind,
};
use base64::Engine as _;
use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use futures::future::Either;
use re_async::AsyncRuntimeHandle;
use rig_core::DynModel;
use rig_core::completion::{
    CompletionRequest, CompletionResponse, FinishReason, Message, ToolDefinition as RigTool,
};
use rig_core::message::{
    DocumentSourceKind, Image, ImageMediaType, MimeType as _, ToolCall, ToolName, ToolResult,
    ToolResultContent, UserContent,
};
use rig_core::operation::Completion;
use rig_core::streaming::{Item, StreamEvent};

use crate::Responder;
use crate::agent_toolbox::{AgentToolbox, ToolOutput};
use crate::backend::{AgentCommand, AgentConnection, AgentEvent, EventSender};

/// The id of the model selector the backend offers as a session config option.
const MODEL_CONFIG_ID: &str = "model";

/// Why a model-API agent could not start, or could not finish a turn.
#[derive(Debug, thiserror::Error)]
pub enum ModelApiError {
    /// The HTTP client that requests are sent through could not be created.
    #[error("Failed to create an HTTP client: {err}")]
    HttpClient { err: reqwest::Error },

    /// The provider reply says that the model failed.
    #[error("The model failed: {message}")]
    Model { message: String },

    /// The request to the provider failed, or its reply could not be read.
    #[error("{}", describe_provider_error(err))]
    Provider { err: Box<rig_core::ProviderError> },
}

/// The id of the one session the backend runs.
const SESSION_ID: &str = "model-api";

/// The permission option that lets a tool call run.
const ALLOW_OPTION_ID: &str = "allow";

/// The permission option that refuses a tool call.
const REJECT_OPTION_ID: &str = "reject";

/// The most model replies one prompt can lead to. Every reply after the first answers tool
/// results, so this bounds how long a model stuck calling tools keeps going (and spending).
/// Viewer tasks take a handful of calls; this leaves ample room for longer ones.
const MAX_REPLIES_PER_TURN: usize = 50;

/// A one-line message for a failed provider request, for the user to read.
///
/// A provider reply is summarized from its HTTP status and the `error.message` in its body,
/// which is where both the Anthropic and the `OpenAI` APIs put the human-readable part.
fn describe_provider_error(err: &rig_core::ProviderError) -> String {
    let Some(status) = err.provider_response_status() else {
        return format!("The model request failed: {err}");
    };

    let code = status.as_u16();
    let summary = match code {
        401 => "The API key was rejected".to_owned(),
        403 => "The API key is not allowed to do this".to_owned(),
        429 => "The model API is rate limiting requests".to_owned(),
        500..=599 => format!("The model API had an internal error ({code})"),
        _ => format!(
            "The model request failed with {code} {}",
            status.canonical_reason().unwrap_or_default()
        ),
    };

    let details = err
        .provider_response_json()
        .ok()
        .flatten()
        .and_then(|json| {
            json.pointer("/error/message")
                .and_then(|message| message.as_str())
                .map(str::to_owned)
        })
        .or_else(|| {
            err.provider_response_body()
                .map(|body| body.trim().to_owned())
        })
        .filter(|details| !details.is_empty());

    match details {
        Some(details) => format!("{summary}: {}", crate::truncate_bytes(&details, 300)),
        None => format!("{summary}."),
    }
}

/// Why a tool call failed. The message is sent back to the model, so that it can correct itself.
#[derive(Debug, thiserror::Error)]
enum ToolCallError {
    #[error("The user declined this tool call")]
    Declined,

    #[error("The arguments are not valid JSON: {details}")]
    InvalidArguments { details: String },

    #[error(transparent)]
    Tool(Box<dyn std::error::Error + Send + Sync>),

    #[error("There is no tool named {name:?}")]
    UnknownTool { name: String },
}

/// A model provider whose API the backend calls directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelApiProvider {
    Anthropic,
    OpenAi,
}

impl ModelApiProvider {
    /// The API key from [`Self::api_key_env_var`], or `None` if it is unset or blank.
    pub fn api_key_from_env(self) -> Option<String> {
        std::env::var(self.api_key_env_var())
            .ok()
            .filter(|key| !key.trim().is_empty())
    }

    /// Human-readable name, e.g. for the panel title.
    pub fn name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
        }
    }

    /// The environment variable the API key is read from.
    pub fn api_key_env_var(self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::OpenAi => "OPENAI_API_KEY",
        }
    }

    /// Where the user creates an API key for this provider.
    ///
    /// An API key is billed by usage, separately from any chat subscription, which cannot be
    /// used here.
    pub fn api_key_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://platform.claude.com/settings/keys",
            Self::OpenAi => "https://platform.openai.com/api-keys",
        }
    }

    /// The model a new session starts with, unless the user has picked another.
    ///
    /// A fast one, like the models `DEFAULT_MODEL_PREFERENCES` picks for agent harnesses,
    /// because the agent panel should feel fast.
    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => rig_core::providers::anthropic::CLAUDE_SONNET_5_5,
            Self::OpenAi => rig_core::providers::openai::GPT_5_MINI,
        }
    }

    /// Models offered in the model selector, best first. Includes [`Self::default_model`].
    fn models(self) -> &'static [&'static str] {
        match self {
            Self::Anthropic => &[
                rig_core::providers::anthropic::CLAUDE_OPUS_5_5,
                rig_core::providers::anthropic::CLAUDE_SONNET_5_5,
                rig_core::providers::anthropic::CLAUDE_HAIKU_4_5,
            ],
            Self::OpenAi => &[
                rig_core::providers::openai::GPT_5_2,
                rig_core::providers::openai::GPT_5_MINI,
            ],
        }
    }
}

/// Everything needed to run an agent loop against a model API.
#[derive(Clone)]
pub struct ModelApiConfig {
    /// Whose API to call.
    pub provider: ModelApiProvider,

    /// Sent to the provider with every request. Never logged.
    pub api_key: String,

    /// The model to start with. The user can switch to another from the model selector.
    pub model: String,

    /// Sent as the system prompt of every request.
    pub system_prompt: Option<String>,

    /// The tools the agent may call. `None` means it can only chat.
    pub toolbox: Option<Arc<dyn AgentToolbox>>,
}

impl std::fmt::Debug for ModelApiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            provider,
            api_key: _,
            model,
            system_prompt,
            toolbox,
        } = self;
        f.debug_struct("ModelApiConfig")
            .field("provider", provider)
            .field("model", model)
            .field("system_prompt", system_prompt)
            .field("toolbox", &toolbox.is_some())
            .finish_non_exhaustive()
    }
}

impl ModelApiConfig {
    fn model(&self, http: &reqwest::Client, model: &str) -> DynModel<Completion> {
        let Self {
            provider, api_key, ..
        } = self;
        let http = rig_reqwest::ReqwestClient::from(http.clone());
        match provider {
            ModelApiProvider::Anthropic => {
                rig_core::providers::anthropic::wire::AnthropicConfig::new(api_key.clone())
                    .connect(http)
                    .completion(model)
                    .erase()
            }
            ModelApiProvider::OpenAi => {
                rig_core::providers::openai::OpenAIConfig::new(api_key.clone())
                    .connect(http)
                    .responses(model)
                    .erase()
            }
        }
    }
}

/// The HTTP client every model of a session sends through, with ring as the TLS provider.
fn http_client() -> Result<reqwest::Client, ModelApiError> {
    // Fails harmlessly when the host already installed a provider.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    reqwest::Client::builder()
        .build()
        .map_err(|err| ModelApiError::HttpClient { err })
}

/// Starts an agent loop on `async_runtime` that calls the provider's API directly.
///
/// `wake` is called after every event so the UI can repaint. Dropping the connection stops the
/// loop, cancelling any turn in progress.
pub fn spawn(
    config: ModelApiConfig,
    async_runtime: &AsyncRuntimeHandle,
    wake: impl Fn() + Send + Sync + 'static,
) -> Result<AgentConnection, ModelApiError> {
    let http = http_client()?;
    let (connection, command_rx, events) = AgentConnection::new(wake);

    let provider_config = config.clone();
    let make_model = move |model: &str| provider_config.model(&http, model);
    async_runtime.spawn_future(async move {
        run(config, make_model, command_rx, &events).await;
        events.send(AgentEvent::Disconnected);
    });

    Ok(connection)
}

/// The model the session sends its turns to, and how to build another one.
struct ModelSelection<F> {
    provider: ModelApiProvider,
    make_model: F,
    id: String,
    model: DynModel<Completion>,
}

impl<F: Fn(&str) -> DynModel<Completion>> ModelSelection<F> {
    fn new(provider: ModelApiProvider, make_model: F, id: String) -> Self {
        let model = make_model(&id);
        Self {
            provider,
            make_model,
            id,
            model,
        }
    }

    fn config_options(&self) -> Vec<SessionConfigOption> {
        let mut models: Vec<SessionConfigSelectOption> = self
            .provider
            .models()
            .iter()
            .map(|model| SessionConfigSelectOption::new(*model, *model))
            .collect();
        if !self.provider.models().contains(&self.id.as_str()) {
            models.insert(
                0,
                SessionConfigSelectOption::new(self.id.clone(), self.id.clone()),
            );
        }
        vec![
            SessionConfigOption::select(
                SessionConfigId::new(MODEL_CONFIG_ID),
                "Model",
                SessionConfigValueId::new(self.id.clone()),
                models,
            )
            .category(SessionConfigOptionCategory::Model),
        ]
    }

    /// Switches models when `config_id` is the model selector, which is the only option offered.
    fn set_config_option(
        &mut self,
        config_id: &SessionConfigId,
        value: &SessionConfigValueId,
        events: &EventSender,
    ) {
        if config_id.0.as_ref() != MODEL_CONFIG_ID {
            events.send(AgentEvent::Error(format!(
                "Unknown session option: {config_id:?}"
            )));
            return;
        }
        self.id = value.0.to_string();
        self.model = (self.make_model)(&self.id);
        events.send(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
            ConfigOptionUpdate::new(self.config_options()),
        )));
    }
}

/// Serves commands until shut down. `make_model` builds the model a model id names.
async fn run(
    config: ModelApiConfig,
    make_model: impl Fn(&str) -> DynModel<Completion>,
    mut command_rx: UnboundedReceiver<AgentCommand>,
    events: &EventSender,
) {
    let mut selection = ModelSelection::new(config.provider, make_model, config.model.clone());
    let mut history: Vec<Message> = Vec::new();

    events.send(AgentEvent::Initialized {
        agent_info: Some(Implementation::new(
            config.provider.name(),
            env!("CARGO_PKG_VERSION"),
        )),
        capabilities: Box::new(
            AgentCapabilities::new().prompt_capabilities(PromptCapabilities::new().image(true)),
        ),
        auth_methods: Vec::new(),
    });
    events.send(AgentEvent::SessionStarted {
        session_id: SessionId::new(SESSION_ID),
        modes: None,
        config_options: selection.config_options(),
    });

    while let Some(command) = command_rx.next().await {
        match command {
            AgentCommand::Prompt(blocks) => {
                history.push(Message::User {
                    content: blocks.into_iter().map(user_content).collect(),
                });

                // Applied once the turn is over, so that a turn runs on one model throughout.
                let mut config_changes = Vec::new();
                let result = {
                    let mut turn = std::pin::pin!(run_turn(
                        selection.model.clone(),
                        config.system_prompt.as_deref(),
                        config.toolbox.as_deref(),
                        &mut history,
                        events,
                    ));
                    loop {
                        match futures::future::select(turn.as_mut(), command_rx.next()).await {
                            Either::Left((result, _)) => break result,
                            Either::Right((None | Some(AgentCommand::Shutdown), _)) => return,
                            Either::Right((Some(AgentCommand::Cancel), _)) => {
                                break Ok(StopReason::Cancelled);
                            }
                            Either::Right((
                                Some(AgentCommand::SetConfigOption { config_id, value }),
                                _,
                            )) => {
                                config_changes.push((config_id, value));
                            }
                            Either::Right((Some(AgentCommand::Prompt(_)), _)) => {
                                events.send(AgentEvent::Error(
                                    "Dropped a prompt sent while the agent was still answering"
                                        .to_owned(),
                                ));
                            }
                            Either::Right((
                                Some(
                                    command @ (AgentCommand::SetMode(_)
                                    | AgentCommand::Authenticate(_)),
                                ),
                                _,
                            )) => {
                                re_log::debug!(
                                    "Ignoring {command:?}: not supported by this backend"
                                );
                            }
                        }
                    }
                };
                events.send(AgentEvent::TurnFinished(
                    result.map_err(|err| err.to_string()),
                ));

                for (config_id, value) in config_changes {
                    selection.set_config_option(&config_id, &value, events);
                }
            }

            AgentCommand::SetConfigOption { config_id, value } => {
                selection.set_config_option(&config_id, &value, events);
            }

            // A running turn handles its own cancellation; between turns, nothing runs.
            AgentCommand::Cancel => {}

            AgentCommand::SetMode(_) | AgentCommand::Authenticate(_) => {
                re_log::debug!("Ignoring {command:?}: not supported by this backend");
            }

            AgentCommand::Shutdown => return,
        }
    }
}

/// Runs one turn: asks the model, runs the tools it calls, and repeats until it stops calling
/// tools.
///
/// Adds replies and tool results to `history` as they arrive, including when the turn fails or
/// is cancelled.
async fn run_turn(
    model: DynModel<Completion>,
    system_prompt: Option<&str>,
    toolbox: Option<&dyn AgentToolbox>,
    history: &mut Vec<Message>,
    events: &EventSender,
) -> Result<StopReason, ModelApiError> {
    let tools = toolbox.map(rig_tools).unwrap_or_default();

    for _ in 0..MAX_REPLIES_PER_TURN {
        let reply = stream_reply(&model, system_prompt, &tools, history, events).await?;
        let calls: Vec<ToolCall> = reply.tool_calls().cloned().collect();
        history.extend(reply.message());

        if calls.is_empty() {
            let stop_reason = match reply.finish_reason() {
                Some(FinishReason::Length) => StopReason::MaxTokens,
                Some(FinishReason::ContentFilter) => StopReason::Refusal,
                _ => StopReason::EndTurn,
            };
            return Ok(stop_reason);
        }

        // Every call needs a result in provider history, even if cancellation interrupts it.
        let results = calls.iter().map(|call| UserContent::ToolResult(ToolResult {
            call: call.id.clone(),
            name: call.function.name.clone(),
            content: vec![ToolResultContent::text(
                "Tool call interrupted; it may have taken effect. Check the state before retrying.",
            )],
            is_error: true,
        })).collect();
        history.push(Message::User { content: results });
        if let Some(Message::User { content: results }) = history.last_mut() {
            for (call, result) in std::iter::zip(&calls, results) {
                *result = run_tool_call(call, toolbox, events).await;
            }
        }
    }

    Ok(StopReason::MaxTurnRequests)
}

/// Streams one reply, forwarding text and reasoning as they arrive.
async fn stream_reply(
    model: &DynModel<Completion>,
    system_prompt: Option<&str>,
    tools: &[Arc<RigTool>],
    history: &[Message],
    events: &EventSender,
) -> Result<CompletionResponse, ModelApiError> {
    let mut request = CompletionRequest::from(history.to_vec()).tools(tools.iter().cloned());
    if let Some(system_prompt) = system_prompt {
        request = request.preamble(system_prompt);
    }

    let mut stream = model
        .stream(request)
        .map_err(|err| ModelApiError::Provider { err: Box::new(err) })?;
    while let Some(item) = stream.next().await {
        match item.map_err(|err| ModelApiError::Provider { err: Box::new(err) })? {
            Item::Event(StreamEvent::Text { text, .. }) => {
                events.send(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
                    text_chunk(text),
                )));
            }
            Item::Event(StreamEvent::Reasoning { text, .. }) => {
                events.send(AgentEvent::Update(SessionUpdate::AgentThoughtChunk(
                    text_chunk(text),
                )));
            }
            Item::Event(_) | Item::Unknown(_) => {}
        }
    }

    let reply = stream
        .finish()
        .await
        .map_err(|err| ModelApiError::Provider { err: Box::new(err) })?;
    // A refusal also carries its explanation as the response error, but it is not a failure.
    if reply.finish_reason() != Some(FinishReason::ContentFilter)
        && let Some(message) = &reply.error
    {
        return Err(ModelApiError::Model {
            message: message.clone(),
        });
    }
    Ok(reply)
}

/// The toolbox's tools, as the model is told about them.
fn rig_tools(toolbox: &dyn AgentToolbox) -> Vec<Arc<RigTool>> {
    toolbox
        .definitions()
        .into_iter()
        .filter_map(|tool| match ToolName::new(tool.name) {
            Ok(name) => Some(Arc::new(RigTool::new(
                name,
                tool.description,
                serde_json::Value::Object(model_api_input_schema(tool.input_schema)),
            ))),
            Err(err) => {
                re_log::warn_once!("Skipping a tool without a name: {err}");
                None
            }
        })
        .collect()
}

/// The keywords that the Anthropic and `OpenAI` APIs reject at the top level of a tool's input
/// schema. They are fine nested inside a property.
const TOP_LEVEL_SCHEMA_COMBINATORS: [&str; 3] = ["allOf", "anyOf", "oneOf"];

/// Drops the [`TOP_LEVEL_SCHEMA_COMBINATORS`] a model API refuses, which would fail every request.
///
/// The constraint they express is lost to validation, so a tool must still check its arguments.
fn model_api_input_schema(
    mut schema: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    for keyword in TOP_LEVEL_SCHEMA_COMBINATORS {
        schema.remove(keyword);
    }
    schema
}

/// Runs one tool call, asking the user first if the tool needs it, and reports it in the
/// transcript. Returns the result to send back to the model.
async fn run_tool_call(
    call: &ToolCall,
    toolbox: Option<&dyn AgentToolbox>,
    events: &EventSender,
) -> UserContent {
    let id = ToolCallId::new(call.id.to_string());
    let name = call.function.name.as_str();
    let arguments = call.function.arguments.clone();
    events.send(AgentEvent::Update(SessionUpdate::ToolCall(
        AcpToolCall::new(id.clone(), name)
            .kind(ToolKind::Other)
            .status(ToolCallStatus::Pending)
            .raw_input(serde_json::Value::Object(arguments.clone())),
    )));

    let outcome = match (&call.function.invalid_arguments, toolbox) {
        (Some(invalid), _) => Err(ToolCallError::InvalidArguments {
            details: invalid.clone(),
        }),
        (None, None) => Err(ToolCallError::UnknownTool {
            name: name.to_owned(),
        }),
        (None, Some(toolbox)) => {
            if toolbox.requires_approval(name) && !is_allowed(&id, name, events).await {
                Err(ToolCallError::Declined)
            } else {
                events.send(status_update(&id, ToolCallStatus::InProgress, Vec::new()));
                toolbox
                    .call(name, arguments)
                    .await
                    .map_err(ToolCallError::Tool)
            }
        }
    };

    let (status, content, is_error) = match outcome {
        Ok(outputs) => (ToolCallStatus::Completed, outputs, false),
        Err(err) => (
            ToolCallStatus::Failed,
            vec![ToolOutput::Text(err.to_string())],
            true,
        ),
    };
    events.send(status_update(
        &id,
        status,
        content.iter().map(acp_content).collect(),
    ));
    UserContent::ToolResult(ToolResult {
        call: call.id.clone(),
        name: call.function.name.clone(),
        content: content.into_iter().map(rig_content).collect(),
        is_error,
    })
}

/// Asks the user whether the call `id` may run, and waits for the answer.
///
/// A cancelled question counts as a refusal.
async fn is_allowed(id: &ToolCallId, name: &str, events: &EventSender) -> bool {
    let (responder, answer) = Responder::channel();
    events.send(AgentEvent::PermissionRequest {
        request: RequestPermissionRequest::new(
            SessionId::new(SESSION_ID),
            ToolCallUpdate::new(
                id.clone(),
                ToolCallUpdateFields::new().title(name.to_owned()),
            ),
            vec![
                PermissionOption::new(ALLOW_OPTION_ID, "Allow", PermissionOptionKind::AllowOnce),
                PermissionOption::new(REJECT_OPTION_ID, "Reject", PermissionOptionKind::RejectOnce),
            ],
        ),
        responder,
    });
    match answer.await {
        Ok(response) => matches!(
            response.outcome,
            RequestPermissionOutcome::Selected(selected) if selected.option_id.0.as_ref() == ALLOW_OPTION_ID
        ),
        Err(_canceled) => false,
    }
}

fn status_update(
    id: &ToolCallId,
    status: ToolCallStatus,
    content: Vec<ContentBlock>,
) -> AgentEvent {
    let mut fields = ToolCallUpdateFields::new().status(status);
    if !content.is_empty() {
        fields = fields.content(content.into_iter().map(Into::into).collect::<Vec<_>>());
    }
    AgentEvent::Update(SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        id.clone(),
        fields,
    )))
}

/// A tool output as the transcript shows it.
fn acp_content(output: &ToolOutput) -> ContentBlock {
    match output {
        ToolOutput::Text(text) => ContentBlock::Text(TextContent::new(text.clone())),
        ToolOutput::Image { bytes, mime_type } => ContentBlock::Image(ImageContent::new(
            base64::engine::general_purpose::STANDARD.encode(bytes),
            mime_type.clone(),
        )),
    }
}

/// A tool output as the model is sent it.
fn rig_content(output: ToolOutput) -> ToolResultContent {
    match output {
        ToolOutput::Text(text) => ToolResultContent::text(text),
        ToolOutput::Image { bytes, mime_type } => ToolResultContent::image_base64(
            base64::engine::general_purpose::STANDARD.encode(bytes),
            ImageMediaType::from_mime_type(&mime_type),
            None,
        ),
    }
}

fn text_chunk(text: String) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text)))
}

/// What the user sent, as the provider takes it. Content the provider has no place for is sent
/// as a textual reference, so the model still knows it was there.
fn user_content(block: ContentBlock) -> UserContent {
    match block {
        ContentBlock::Text(text) => UserContent::text(text.text),
        ContentBlock::Image(image) => UserContent::Image(Image {
            data: DocumentSourceKind::base64(image.data),
            media_type: ImageMediaType::from_mime_type(&image.mime_type),
            ..Default::default()
        }),
        ContentBlock::ResourceLink(link) => UserContent::text(link.uri),
        other => UserContent::text(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::LocalPool;
    use futures::task::LocalSpawnExt as _;
    use rig_core::test_utils::{MockCompletionModel, MockStreamEvent};

    use agent_client_protocol_schema::v1::{RequestPermissionResponse, SelectedPermissionOutcome};

    use super::*;

    #[test]
    fn input_schema_drops_only_top_level_combinators() -> serde_json::Result<()> {
        let nested = serde_json::json!({ "oneOf": [{ "type": "string" }, { "type": "integer" }] });
        let schema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": { "target": nested },
            "oneOf": [{ "required": ["a"] }, { "required": ["b"] }],
            "allOf": [],
            "anyOf": [],
        }))?;
        assert_eq!(
            serde_json::Value::Object(model_api_input_schema(schema)),
            serde_json::json!({ "type": "object", "properties": { "target": nested } })
        );
        Ok(())
    }

    fn provider_error(status: u16, body: &str) -> String {
        let status = http::StatusCode::from_u16(status).expect("valid status");
        let err = rig_core::ProviderError::from_http_response(status, body);
        ModelApiError::Provider { err: Box::new(err) }.to_string()
    }

    #[test]
    fn provider_errors_read_well() {
        assert_eq!(
            provider_error(
                401,
                r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},"request_id":"req_1"}"#,
            ),
            "The API key was rejected: invalid x-api-key"
        );
        assert_eq!(
            provider_error(429, ""),
            "The model API is rate limiting requests."
        );
        assert_eq!(
            provider_error(400, "  not json  "),
            "The model request failed with 400 Bad Request: not json"
        );
    }

    /// Echoes its arguments. `echo` runs freely; `delete` needs approval.
    struct TestToolbox;

    impl AgentToolbox for TestToolbox {
        fn definitions(&self) -> Vec<crate::ToolDefinition> {
            ["echo", "delete"]
                .into_iter()
                .map(|name| crate::ToolDefinition {
                    name: name.to_owned(),
                    description: format!("The {name} tool"),
                    input_schema: serde_json::Map::new(),
                })
                .collect()
        }

        fn requires_approval(&self, name: &str) -> bool {
            name == "delete"
        }

        fn call(
            &self,
            name: &str,
            arguments: serde_json::Map<String, serde_json::Value>,
        ) -> futures::future::BoxFuture<
            'static,
            Result<Vec<ToolOutput>, Box<dyn std::error::Error + Send + Sync>>,
        > {
            let text = format!("{name} {}", serde_json::Value::Object(arguments));
            Box::pin(async move { Ok(vec![ToolOutput::Text(text)]) })
        }
    }

    /// Runs the loop on `model`, feeding it one prompt at a time and waiting for each turn.
    /// Allows every tool call that asks for permission if `allow`, else rejects it.
    fn run_prompts(
        model: &MockCompletionModel,
        toolbox: Option<Arc<dyn AgentToolbox>>,
        allow: bool,
        prompts: &[&str],
    ) -> Vec<AgentEvent> {
        let config = ModelApiConfig {
            provider: ModelApiProvider::Anthropic,
            api_key: String::new(),
            model: "mock".to_owned(),
            system_prompt: Some("Be brief".to_owned()),
            toolbox,
        };
        let (command_tx, command_rx) = futures::channel::mpsc::unbounded();
        let (events, event_rx) = EventSender::new(|| {});

        let mut pool = LocalPool::new();
        let model = model.clone();
        pool.spawner()
            .spawn_local(async move {
                let make_model = move |_: &str| model.clone().erase();
                run(config, make_model, command_rx, &events).await;
            })
            .expect("spawn the loop");

        let mut received = Vec::new();
        let answer_permissions = |received: &mut Vec<AgentEvent>| {
            for event in event_rx.try_iter() {
                if let AgentEvent::PermissionRequest { responder, .. } = event {
                    let option = if allow {
                        ALLOW_OPTION_ID
                    } else {
                        REJECT_OPTION_ID
                    };
                    responder
                        .respond(RequestPermissionResponse::new(
                            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                                option,
                            )),
                        ))
                        .expect("the loop waits for the answer");
                } else {
                    received.push(event);
                }
            }
        };
        for prompt in prompts {
            command_tx
                .unbounded_send(AgentCommand::Prompt(vec![ContentBlock::Text(
                    TextContent::new(*prompt),
                )]))
                .expect("loop running");
            loop {
                pool.run_until_stalled();
                let before = received.len();
                answer_permissions(&mut received);
                if received[before..]
                    .iter()
                    .any(|event| matches!(event, AgentEvent::TurnFinished(_)))
                {
                    break;
                }
            }
        }
        command_tx
            .unbounded_send(AgentCommand::Shutdown)
            .expect("loop running");
        pool.run_until_stalled();
        answer_permissions(&mut received);

        received
    }

    fn tool_results(request: &CompletionRequest) -> Vec<&ToolResult> {
        request
            .chat_history
            .iter()
            .filter_map(|message| match message.as_ref() {
                Message::User { content } => Some(content),
                Message::System { .. } | Message::Assistant(_) => None,
            })
            .flatten()
            .filter_map(|content| match content {
                UserContent::ToolResult(result) => Some(result),
                _ => None,
            })
            .collect()
    }

    fn tool_call_then_text(tool: &str) -> MockCompletionModel {
        MockCompletionModel::from_stream_turns([
            vec![
                MockStreamEvent::tool_call("call-1", tool, serde_json::json!({ "x": 1 })),
                MockStreamEvent::final_response_with_default_usage(),
            ],
            vec![
                MockStreamEvent::text("Done"),
                MockStreamEvent::final_response_with_default_usage(),
            ],
        ])
    }

    #[test]
    fn runs_a_tool_and_sends_the_result_back() {
        let model = tool_call_then_text("echo");

        let events = run_prompts(&model, Some(Arc::new(TestToolbox)), false, &["Go"]);

        assert_eq!(message_text(&events), "Done");
        let requests = model.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].tools.len(), 2);
        let results = tool_results(&requests[1]);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
        assert_eq!(
            results[0].content,
            [ToolResultContent::text(r#"echo {"x":1}"#)]
        );
    }

    #[test]
    fn a_failed_turn_keeps_the_tools_that_already_ran() {
        let model = MockCompletionModel::from_stream_turns([
            vec![
                MockStreamEvent::tool_call("call-1", "echo", serde_json::json!({ "x": 1 })),
                MockStreamEvent::final_response_with_default_usage(),
            ],
            vec![MockStreamEvent::error("overloaded")],
            vec![
                MockStreamEvent::text("ok"),
                MockStreamEvent::final_response_with_default_usage(),
            ],
        ]);

        run_prompts(&model, Some(Arc::new(TestToolbox)), false, &["Go", "Retry"]);

        let requests = model.requests();
        assert_eq!(requests.len(), 3);
        assert_eq!(tool_results(&requests[2]).len(), 1);
    }

    #[test]
    fn cancelled_tools_keep_completed_and_interrupted_results() {
        use futures::FutureExt as _;

        let model = MockCompletionModel::from_stream_turns([vec![
            MockStreamEvent::tool_call("call-1", "echo", serde_json::json!({})),
            MockStreamEvent::tool_call("call-2", "delete", serde_json::json!({})),
            MockStreamEvent::final_response_with_default_usage(),
        ]]);
        let (events, event_rx) = EventSender::new(|| {});
        let mut history = vec![Message::user("Go")];

        assert!(
            run_turn(
                model.erase(),
                None,
                Some(&TestToolbox),
                &mut history,
                &events
            )
            .now_or_never()
            .is_none(),
            "the second tool must wait for permission"
        );
        assert!(
            event_rx
                .try_iter()
                .any(|event| matches!(event, AgentEvent::PermissionRequest { .. }))
        );

        let request = CompletionRequest::from(history);
        let results = tool_results(&request);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, [ToolResultContent::text("echo {}")]);
        assert!(!results[0].is_error);
        assert!(results[1].is_error);
        assert_eq!(results[1].call.to_string(), "call-2");
    }

    #[test]
    fn a_declined_tool_call_is_reported_to_the_model() {
        let model = tool_call_then_text("delete");

        run_prompts(&model, Some(Arc::new(TestToolbox)), false, &["Go"]);

        let requests = model.requests();
        let results = tool_results(&requests[1]);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_error, "a declined call must fail");
        assert_eq!(
            results[0].content,
            [ToolResultContent::text("The user declined this tool call")]
        );
    }

    #[test]
    fn an_approved_tool_call_runs() {
        let model = tool_call_then_text("delete");

        run_prompts(&model, Some(Arc::new(TestToolbox)), true, &["Go"]);

        let requests = model.requests();
        let results = tool_results(&requests[1]);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].content,
            [ToolResultContent::text(r#"delete {"x":1}"#)]
        );
    }

    fn message_text(events: &[AgentEvent]) -> String {
        events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::Update(SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(text),
                    ..
                })) => Some(text.text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn turn_results(events: &[AgentEvent]) -> Vec<&Result<StopReason, String>> {
        events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TurnFinished(result) => Some(result),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn default_models_are_the_preferred_fast_ones() {
        for provider in [ModelApiProvider::Anthropic, ModelApiProvider::OpenAi] {
            let model = provider.default_model();
            assert!(provider.models().contains(&model), "{model:?}");
            assert!(
                crate::settings::DEFAULT_MODEL_PREFERENCES
                    .iter()
                    .any(|preference| model.contains(preference)),
                "{model:?} is not one of DEFAULT_MODEL_PREFERENCES"
            );
        }
    }

    #[test]
    fn streams_the_reply_and_remembers_it_for_the_next_turn() {
        let model = MockCompletionModel::from_stream_turns([
            vec![
                MockStreamEvent::text("Hi"),
                MockStreamEvent::text(" there"),
                MockStreamEvent::final_response_with_default_usage(),
            ],
            vec![
                MockStreamEvent::text("!"),
                MockStreamEvent::final_response_with_default_usage(),
            ],
        ]);

        let events = run_prompts(&model, None, false, &["Hi", "Again"]);

        assert_eq!(message_text(&events), "Hi there!");
        assert!(matches!(
            turn_results(&events)[..],
            [Ok(StopReason::EndTurn), Ok(StopReason::EndTurn)]
        ));

        let requests = model.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].system_instructions(), Some("Be brief"));
        let roles: Vec<&str> = requests[1]
            .chat_history
            .iter()
            .filter_map(|message| match message.as_ref() {
                Message::System { .. } => None,
                Message::User { .. } => Some("user"),
                Message::Assistant(_) => Some("assistant"),
            })
            .collect();
        assert_eq!(roles, ["user", "assistant", "user"]);
    }

    #[test]
    fn a_refusal_is_not_an_error() {
        let model = MockCompletionModel::from_stream_turns([vec![MockStreamEvent::FinalResponse(
            rig_core::operation::Finish {
                reason: Some(FinishReason::ContentFilter),
                error: Some("The model refused".to_owned()),
                ..Default::default()
            },
        )]]);

        let events = run_prompts(&model, None, false, &["Hi"]);

        assert!(matches!(
            turn_results(&events)[..],
            [Ok(StopReason::Refusal)]
        ));
    }

    #[test]
    fn a_provider_error_fails_the_turn_but_keeps_the_session() {
        let model = MockCompletionModel::from_stream_turns([
            vec![MockStreamEvent::error("overloaded")],
            vec![
                MockStreamEvent::text("ok"),
                MockStreamEvent::final_response_with_default_usage(),
            ],
        ]);

        let events = run_prompts(&model, None, false, &["Hi", "Retry"]);

        let results = turn_results(&events);
        assert_eq!(results.len(), 2);
        assert!(
            results[0]
                .as_ref()
                .is_err_and(|err| err.contains("overloaded"))
        );
        assert!(matches!(results[1], Ok(StopReason::EndTurn)));
        assert_eq!(message_text(&events), "ok");
    }
}
