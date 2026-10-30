use re_agent::RedactionAgent;
use re_agent::acp::schema::v1::{ContentBlock, ToolCallContent, ToolCallStatus};
use re_agent::{
    AgentSession, ToolCallState, Transcript, TranscriptItem, message_text, truncate_bytes,
};
use re_sdk_types::external::uuid::Uuid;
use re_ui::ButtonExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};
use serde::Serialize;

/// The ATIF version the trajectory follows.
///
/// ATIF is the Agent Trajectory Interchange Format of the Harbor eval framework:
/// <https://docs.harborframework.com/agents/atif>.
const SCHEMA_VERSION: &str = "ATIF-v1.7";

/// Tool output longer than this is cut, so one large output cannot take most of the trajectory.
///
/// At this size, about 90 cut outputs fit in `MAX_TRAJECTORY_BYTES`.
const MAX_OBSERVATION_BYTES: usize = 10_000;

/// Largest trajectory JSON sent with the analytics event.
///
/// `PostHog` drops events larger than about 1 MB. This leaves room for the other event properties.
const MAX_TRAJECTORY_BYTES: usize = 900_000;

/// Where this feedback comes from, as the `source` of the analytics event.
const FEEDBACK_SOURCE: &str = "viewer_panel";

/// Asks the user for feedback on one agent conversation,
/// then sends it together with the conversation as an ATIF trajectory.
#[derive(Default)]
pub struct FeedbackModal {
    modal: ModalHandler,
    trajectory: Option<Trajectory>,
    text: String,

    agent: Option<RedactionAgent>,

    redactor: re_agent::Redactor,
}

impl FeedbackModal {
    /// Takes a copy of the conversation as it is at this call.
    /// Later turns are not part of the feedback.
    /// `agent` is the agent to redact the feedback with.
    pub fn open(&mut self, session: &AgentSession, agent: Option<RedactionAgent>) {
        self.trajectory = Some(Trajectory::new(session.transcript(), session.agent_name()));
        self.agent = agent;
        self.text.clear();
        self.modal.open();
    }

    /// Must be called every frame, also when the modal is closed.
    ///
    /// Sending needs analytics to be enabled, and the Send button is disabled otherwise.
    pub fn ui(&mut self, ctx: &egui::Context) {
        crate::agent_analytics::update_redactor(&mut self.redactor, ctx);

        let Self { modal, text, .. } = self;
        let send_clicked = modal.ui(
            ctx,
            || ModalWrapper::new("Send feedback").min_width(400.0),
            |ui| {
                ui.label("How did the agent do? What should it have done instead?");
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
                let can_send = re_analytics::is_enabled();
                ui.weak(if can_send {
                    "Your feedback is sent to Rerun together with the agent trace: \
                     your prompts, the agent's answers, and its tool calls. \
                     Another agent session redacts any sensitive information first."
                } else {
                    "Feedback is sent with analytics, and analytics is turned off."
                });
                ui.add_space(8.0);
                ui.add_enabled(
                    can_send && !text.trim().is_empty(),
                    egui::Button::new("Send").primary().size_small(),
                )
                .clicked()
            },
        );

        if send_clicked == Some(true)
            && let Some(mut trajectory) = self.trajectory.take()
        {
            trajectory.extra.feedback = std::mem::take(&mut self.text);
            let agent = self.agent.take();
            self.send(trajectory, agent);
            self.modal.close();
        }
    }

    fn send(&mut self, trajectory: Trajectory, redact_with: Option<RedactionAgent>) {
        let Some(redact_with) = redact_with else {
            re_log::warn!(
                "Your feedback was not sent: no agent is available to remove sensitive data from it."
            );
            return;
        };
        let json = match serde_json::to_value(&trajectory) {
            Ok(json) => json,
            Err(err) => {
                re_log::error!("Your feedback was not sent: {err}");
                return;
            }
        };
        let agent = trajectory.agent.name;
        let image_count = trajectory.image_count;
        re_log::info!("Removing sensitive data from your feedback. It is sent after that.");
        self.redactor.redact(
            json,
            redact_with,
            Box::new(move |redacted| match redacted {
                Ok(mut json) => {
                    // Added after redaction, because the redactor replaces ids with `<id>`.
                    let feedback_id = Uuid::new_v4();
                    re_analytics::record(|| {
                        feedback_event(&mut json, feedback_id, agent, image_count)
                    });
                    re_log::info!("Thank you for your feedback.");
                }
                Err(err) => re_log::warn!(
                    "Your feedback was not sent: the sensitive data could not be removed ({err})."
                ),
            }),
        );
    }
}

/// The analytics event for a redacted trajectory.
///
/// Also writes `feedback_id` into the trajectory's `extra`, so the event and the trajectory can be
/// joined wherever the trajectory is stored.
fn feedback_event(
    trajectory: &mut serde_json::Value,
    feedback_id: Uuid,
    agent: String,
    image_count: usize,
) -> re_analytics::event::Feedback {
    if let Some(extra) = trajectory
        .get_mut("extra")
        .and_then(serde_json::Value::as_object_mut)
    {
        extra.insert("feedback_id".to_owned(), feedback_id.to_string().into());
    }
    let feedback = trajectory
        .pointer("/extra/feedback")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let json = trajectory.to_string();
    re_analytics::event::Feedback {
        feedback_id,
        author: re_analytics::event::FeedbackAuthor::User,
        source: FEEDBACK_SOURCE,
        agent,
        about: re_analytics::event::FeedbackAbout::Session(re_analytics::event::SessionFeedback {
            feedback,
            trajectory_bytes: Some(json.len()),
            trajectory: (json.len() <= MAX_TRAJECTORY_BYTES).then_some(json),
            image_count: Some(image_count),
        }),
    }
}

#[derive(Serialize)]
struct Trajectory {
    schema_version: &'static str,
    agent: Agent,
    steps: Vec<Step>,
    extra: Extra,

    /// Images in the conversation, which the steps do not carry.
    #[serde(skip)]
    image_count: usize,
}

#[derive(Serialize)]
struct Agent {
    name: String,
    version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    model_name: Option<String>,
}

/// Rerun's own fields, in the ATIF `extra` object.
///
/// `feedback_id` is added to the redacted JSON, see [`feedback_event`].
#[derive(Serialize)]
struct Extra {
    feedback: String,
    rerun_version: &'static str,
}

#[derive(Serialize)]
struct Step {
    /// Counts from 1, in transcript order.
    step_id: usize,

    /// Serialized as RFC 3339, in UTC.
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<jiff::Timestamp>,

    source: Source,
    message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<ToolCall>,

    /// One result for each entry in `tool_calls`, in the same order.
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<Observation>,
}

#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Source {
    User,
    Agent,
    System,
}

#[derive(Serialize)]
struct ToolCall {
    tool_call_id: String,

    /// The ACP tool call title, e.g. `Read app.ron`. ACP reports no function name.
    function_name: String,

    /// The raw tool input, or `{}` when the agent sent none.
    arguments: serde_json::Value,
}

#[derive(Serialize)]
struct Observation {
    results: Vec<ObservationResult>,
}

#[derive(Serialize)]
struct ObservationResult {
    /// The `tool_call_id` of the call this is the output of.
    source_call_id: String,

    /// The tool output cut to `MAX_OBSERVATION_BYTES`, with a `Failed: ` prefix if the call failed.
    content: String,
}

impl Trajectory {
    fn new(transcript: &Transcript, agent_name: Option<&str>) -> Self {
        let mut steps: Vec<Step> = Vec::new();
        let mut image_count = 0;
        for entry in &transcript.items {
            match &entry.item {
                TranscriptItem::User(prompt) => {
                    image_count += prompt.images.len();
                    steps.push(Step::new(
                        Source::User,
                        prompt.text.clone(),
                        entry.created_at,
                    ));
                }
                TranscriptItem::Agent { content, thoughts } => {
                    image_count += content.iter().filter(|block| is_image(block)).count();
                    let mut step =
                        Step::new(Source::Agent, message_text(content), entry.created_at);
                    step.reasoning_content = (!thoughts.is_empty()).then(|| thoughts.clone());
                    steps.push(step);
                }
                TranscriptItem::ToolCall(call) => {
                    image_count += call
                        .content
                        .iter()
                        .filter(|content| {
                            matches!(content, ToolCallContent::Content(content) if is_image(&content.content))
                        })
                        .count();
                    if steps.last().is_none_or(|step| step.source != Source::Agent) {
                        steps.push(Step::new(Source::Agent, String::new(), entry.created_at));
                    }
                    if let Some(step) = steps.last_mut() {
                        step.push_tool_call(call);
                    }
                }
                TranscriptItem::McpStartupFailure(failure) => {
                    steps.push(Step::new(
                        Source::System,
                        format!(
                            "MCP server `{}` failed to start: {}",
                            failure.server_name, failure.error
                        ),
                        entry.created_at,
                    ));
                }
                TranscriptItem::Note { text, .. } => {
                    steps.push(Step::new(Source::System, text.clone(), entry.created_at));
                }
            }
        }
        for (index, step) in steps.iter_mut().enumerate() {
            step.step_id = index + 1;
        }

        Self {
            schema_version: SCHEMA_VERSION,
            agent: Agent {
                name: agent_name.unwrap_or("unknown").to_owned(),
                version: "unknown".to_owned(),
                model_name: transcript.current_model().map(str::to_owned),
            },
            steps,
            extra: Extra {
                feedback: String::new(),
                rerun_version: env!("CARGO_PKG_VERSION"),
            },
            image_count,
        }
    }
}

/// `time` as a [`jiff::Timestamp`], or `None` when it is out of range.
fn jiff_timestamp(time: web_time::SystemTime) -> Option<jiff::Timestamp> {
    let since_epoch = time.duration_since(web_time::UNIX_EPOCH).ok()?;
    let nanoseconds = i128::try_from(since_epoch.as_nanos()).ok()?;
    jiff::Timestamp::from_nanosecond(nanoseconds).ok()
}

impl Step {
    fn new(source: Source, message: String, created_at: web_time::SystemTime) -> Self {
        Self {
            step_id: 0,
            timestamp: jiff_timestamp(created_at),
            source,
            message,
            reasoning_content: None,
            tool_calls: Vec::new(),
            observation: None,
        }
    }

    fn push_tool_call(&mut self, call: &ToolCallState) {
        let id = call.id.to_string();
        self.tool_calls.push(ToolCall {
            tool_call_id: id.clone(),
            function_name: call.title.clone(),
            arguments: call
                .raw_input
                .clone()
                .unwrap_or_else(|| serde_json::json!({})),
        });
        self.observation
            .get_or_insert_with(|| Observation {
                results: Vec::new(),
            })
            .results
            .push(ObservationResult {
                source_call_id: id,
                content: if call.status == ToolCallStatus::Failed {
                    format!("Failed: {}", tool_output(call))
                } else {
                    tool_output(call)
                },
            });
    }
}

fn is_image(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::Image(_))
}

fn tool_output(call: &ToolCallState) -> String {
    let output = match &call.raw_output {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(value) => value.to_string(),
        None => call
            .content
            .iter()
            .filter_map(|content| match content {
                ToolCallContent::Content(content) => match &content.content {
                    ContentBlock::Text(text) => Some(text.text.as_str()),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    };
    truncate_bytes(&output, MAX_OBSERVATION_BYTES)
}

#[cfg(test)]
mod tests {
    use re_agent::acp::schema::v1::{
        ContentChunk, SessionUpdate, TextContent, ToolCall as AcpToolCall,
    };

    use super::*;

    fn text(text: &str) -> ContentBlock {
        ContentBlock::Text(TextContent::new(text.to_owned()))
    }

    #[test]
    fn trajectory_snapshot() {
        let mut transcript = Transcript::default();
        transcript.push_user("What redap servers do we have?".into());
        transcript.apply(SessionUpdate::AgentThoughtChunk(ContentChunk::new(text(
            "The saved state lists them.",
        ))));
        transcript.apply(SessionUpdate::AgentMessageChunk(ContentChunk::new(text(
            "Let me look.",
        ))));
        transcript.apply(SessionUpdate::ToolCall(
            AcpToolCall::new("call-1", "Read app.ron")
                .status(ToolCallStatus::Completed)
                .raw_input(serde_json::json!({ "path": "app.ron" }))
                .raw_output(serde_json::json!(
                    "redap_servers: [\"rerun+http://127.0.0.1:51234\"]"
                )),
        ));
        transcript.apply(SessionUpdate::ToolCall(
            AcpToolCall::new("call-2", "Fetch")
                .status(ToolCallStatus::Failed)
                .raw_output(serde_json::json!("timeout")),
        ));
        transcript.apply(SessionUpdate::AgentMessageChunk(ContentChunk::new(text(
            "There is one local server.",
        ))));
        transcript.push_note("Turn cancelled", false);

        let start = web_time::UNIX_EPOCH + std::time::Duration::from_hours(500_000);
        for (index, entry) in transcript.items.iter_mut().enumerate() {
            entry.created_at = start + std::time::Duration::from_secs(index as u64);
        }

        let mut trajectory = Trajectory::new(&transcript, Some("Test Agent"));
        trajectory.extra.feedback = "It should have asked the viewer.".to_owned();
        trajectory.extra.rerun_version = "0.0.0";
        insta::assert_snapshot!(
            "trajectory",
            serde_json::to_string_pretty(&trajectory).expect("serialize")
        );
    }

    #[test]
    fn long_tool_output_is_cut() {
        let mut transcript = Transcript::default();
        transcript.push_user("Read it all.".into());
        transcript.apply(SessionUpdate::ToolCall(
            AcpToolCall::new("call-1", "Read")
                .status(ToolCallStatus::Completed)
                .raw_output(serde_json::json!("x".repeat(MAX_OBSERVATION_BYTES + 10))),
        ));

        let trajectory = Trajectory::new(&transcript, None);
        let content = &trajectory.steps[1]
            .observation
            .as_ref()
            .expect("an observation")
            .results[0]
            .content;
        assert_eq!(
            *content,
            truncate_bytes(
                &"x".repeat(MAX_OBSERVATION_BYTES + 10),
                MAX_OBSERVATION_BYTES
            )
        );
    }

    fn session(event: re_analytics::event::Feedback) -> re_analytics::event::SessionFeedback {
        match event.about {
            re_analytics::event::FeedbackAbout::Session(session) => session,
            re_analytics::event::FeedbackAbout::Product(_) => panic!("expected session feedback"),
        }
    }

    #[test]
    fn a_trajectory_too_large_to_send_is_left_out() {
        let mut transcript = Transcript::default();
        for _ in 0..=(MAX_TRAJECTORY_BYTES / MAX_OBSERVATION_BYTES) {
            transcript.apply(SessionUpdate::ToolCall(
                AcpToolCall::new(format!("call-{}", transcript.items.len()), "Read")
                    .raw_output(serde_json::json!("x".repeat(MAX_OBSERVATION_BYTES))),
            ));
        }
        let mut trajectory = Trajectory::new(&transcript, None);
        trajectory.extra.feedback = "Too slow".to_owned();

        let mut json = serde_json::to_value(&trajectory).expect("serialize");
        let event = session(feedback_event(
            &mut json,
            Uuid::new_v4(),
            "Test Agent".to_owned(),
            0,
        ));
        assert_eq!(event.feedback, "Too slow");
        assert!(MAX_TRAJECTORY_BYTES < event.trajectory_bytes.expect("a trajectory"));
        assert!(event.trajectory.is_none());

        let mut small =
            serde_json::to_value(Trajectory::new(&Transcript::default(), None)).expect("serialize");
        let small = session(feedback_event(
            &mut small,
            Uuid::new_v4(),
            "Test Agent".to_owned(),
            0,
        ));
        let sent: serde_json::Value =
            serde_json::from_str(&small.trajectory.expect("a small trajectory is sent"))
                .expect("parse");
        assert_eq!(sent["schema_version"], SCHEMA_VERSION);
    }

    #[test]
    fn the_event_and_the_trajectory_share_the_feedback_id() {
        let mut json =
            serde_json::to_value(Trajectory::new(&Transcript::default(), None)).expect("serialize");
        let feedback_id = Uuid::new_v4();
        let event = feedback_event(&mut json, feedback_id, "Test Agent".to_owned(), 2);
        assert_eq!(event.feedback_id, feedback_id);
        assert_eq!(event.source, FEEDBACK_SOURCE);
        assert_eq!(event.author, re_analytics::event::FeedbackAuthor::User);
        let event = session(event);
        assert_eq!(event.image_count, Some(2));

        let sent: serde_json::Value =
            serde_json::from_str(&event.trajectory.expect("a small trajectory is sent"))
                .expect("parse");
        assert_eq!(sent["extra"]["feedback_id"], feedback_id.to_string());
    }

    #[test]
    fn images_are_counted_but_not_sent() {
        use re_agent::acp::schema::v1::ImageContent;
        use re_agent::{Prompt, PromptImage};

        let mut transcript = Transcript::default();
        transcript.push_user(Prompt {
            text: "What is wrong here?".to_owned(),
            images: vec![PromptImage {
                bytes: std::sync::Arc::from([0_u8].as_slice()),
                mime_type: "image/png".to_owned(),
                resolution: [800, 600],
            }],
        });
        transcript.apply(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::Image(ImageContent::new("AAAA", "image/png")),
        )));

        let trajectory = Trajectory::new(&transcript, None);
        assert_eq!(trajectory.image_count, 2);
        let json = serde_json::to_string(&trajectory).expect("serialize");
        assert!(!json.contains("AAAA"), "{json}");
        assert!(!json.contains("image_count"), "{json}");
    }
}
