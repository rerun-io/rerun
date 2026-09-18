use std::collections::HashMap;

use agent_client_protocol::schema::v1::{
    AvailableCommand, ContentBlock, Plan, SessionConfigKind, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOption, SessionConfigSelectOptions,
    SessionModeId, SessionUpdate, TextContent, ToolCall, ToolCallContent, ToolCallId,
    ToolCallLocation, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind, UsageUpdate,
};

/// The accumulated state of one tool call, patched by later updates.
#[derive(Clone, Debug)]
pub struct ToolCallState {
    /// Agent-chosen id that later updates refer to.
    pub id: ToolCallId,

    /// Human-readable description of the call, e.g. "Read src/main.rs".
    pub title: String,

    /// What sort of tool this is (read, edit, execute, …). Picks the icon.
    pub kind: ToolKind,

    /// Latest reported status, from pending to completed or failed.
    pub status: ToolCallStatus,

    /// Output the agent chose to show: text, diffs, terminal output.
    pub content: Vec<ToolCallContent>,

    /// Files (and lines) the call touched.
    pub locations: Vec<ToolCallLocation>,

    /// The arguments as the agent sent them to the tool, if the agent shares them.
    pub raw_input: Option<serde_json::Value>,

    /// The tool's result as the agent got it, if the agent shares it.
    pub raw_output: Option<serde_json::Value>,
}

impl ToolCallState {
    fn from_call(call: ToolCall) -> Self {
        let ToolCall {
            tool_call_id,
            title,
            kind,
            status,
            content,
            locations,
            raw_input,
            raw_output,
            ..
        } = call;
        Self {
            id: tool_call_id,
            title,
            kind,
            status,
            content,
            locations,
            raw_input,
            raw_output,
        }
    }

    fn from_update(update: ToolCallUpdate) -> Self {
        let mut state = Self {
            id: update.tool_call_id.clone(),
            title: String::new(),
            kind: ToolKind::Other,
            status: ToolCallStatus::Pending,
            content: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            raw_output: None,
        };
        state.apply(update.fields);
        state
    }

    pub fn apply(&mut self, fields: ToolCallUpdateFields) {
        let ToolCallUpdateFields {
            kind,
            status,
            title,
            content,
            locations,
            raw_input,
            raw_output,
            ..
        } = fields;

        if let Some(kind) = kind {
            self.kind = kind;
        }
        if let Some(status) = status {
            self.status = status;
        }
        if let Some(title) = title {
            self.title = title;
        }
        if let Some(content) = content {
            self.content = content;
        }
        if let Some(locations) = locations {
            self.locations = locations;
        }
        if let Some(raw_input) = raw_input {
            self.raw_input = Some(raw_input);
        }
        if let Some(raw_output) = raw_output {
            self.raw_output = Some(raw_output);
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(
            self.status,
            ToolCallStatus::Completed | ToolCallStatus::Failed
        )
    }
}

/// One entry in the conversation, in display order.
#[derive(Clone, Debug)]
pub enum TranscriptItem {
    User {
        text: String,
    },

    Agent {
        /// Consecutive text blocks are merged so they render as one markdown document.
        content: Vec<ContentBlock>,
        thoughts: String,
    },

    ToolCall(ToolCallState),

    /// Something the UI wants to say inline: an error, a cancelled turn, …
    Note {
        text: String,
        is_error: bool,
    },
}

/// A transcript item plus when it first appeared.
#[derive(Clone, Debug)]
pub struct TranscriptEntry {
    /// When the item was first added. Later updates to the item do not move it.
    pub created_at: std::time::SystemTime,

    /// The item itself.
    pub item: TranscriptItem,
}

/// Everything that has happened in a session, in display order.
#[derive(Default)]
pub struct Transcript {
    /// Messages, tool calls, and notes in the order they appeared.
    pub items: Vec<TranscriptEntry>,

    /// Index into [`Self::items`] for each tool call.
    tool_calls: HashMap<ToolCallId, usize>,

    /// The agent's latest plan, if it published one.
    pub plan: Option<Plan>,

    /// Slash commands the agent accepts, as of its latest update.
    pub available_commands: Vec<AvailableCommand>,

    /// The permission mode the agent reports being in.
    pub current_mode: Option<SessionModeId>,

    /// The agent's configuration, e.g. its model selector, as of its latest update.
    pub config_options: Vec<SessionConfigOption>,

    /// Context-window usage the agent last reported.
    pub usage: Option<UsageUpdate>,

    /// Title the agent gave the session, if any.
    pub title: Option<String>,
}

impl Transcript {
    fn push(&mut self, item: TranscriptItem) {
        self.items.push(TranscriptEntry {
            created_at: std::time::SystemTime::now(),
            item,
        });
    }

    pub fn push_user(&mut self, text: String) {
        self.push(TranscriptItem::User { text });
    }

    pub fn push_note(&mut self, text: impl Into<String>, is_error: bool) {
        self.push(TranscriptItem::Note {
            text: text.into(),
            is_error,
        });
    }

    /// The name of the model the agent says it is using, if it offers a model selector.
    pub fn current_model(&self) -> Option<&str> {
        let selector = self
            .config_options
            .iter()
            .find(|option| option.category == Some(SessionConfigOptionCategory::Model))?;
        let SessionConfigKind::Select(select) = &selector.kind else {
            return None;
        };
        let mut models: Box<dyn Iterator<Item = &SessionConfigSelectOption>> = match &select.options
        {
            SessionConfigSelectOptions::Ungrouped(models) => Box::new(models.iter()),
            SessionConfigSelectOptions::Grouped(groups) => {
                Box::new(groups.iter().flat_map(|group| &group.options))
            }
            _ => return None,
        };

        // An agent may report a value it does not offer as an option; show it raw rather than
        // claim it has no model.
        Some(
            models
                .find(|model| model.value == select.current_value)
                .map_or_else(
                    || select.current_value.0.as_ref(),
                    |model| model.name.as_str(),
                ),
        )
    }

    pub fn tool_call(&self, id: &ToolCallId) -> Option<&ToolCallState> {
        let index = *self.tool_calls.get(id)?;
        match &self.items.get(index)?.item {
            TranscriptItem::ToolCall(call) => Some(call),
            _ => None,
        }
    }

    pub fn apply(&mut self, update: SessionUpdate) {
        match update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                push_merged(self.agent_item().0, chunk.content);
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    self.agent_item().1.push_str(&text.text);
                }
            }
            SessionUpdate::ToolCall(call) => {
                let id = call.tool_call_id.clone();
                if let Some(&index) = self.tool_calls.get(&id) {
                    if let Some(TranscriptEntry {
                        item: TranscriptItem::ToolCall(existing),
                        ..
                    }) = self.items.get_mut(index)
                    {
                        *existing = ToolCallState::from_call(call);
                    }
                } else {
                    self.tool_calls.insert(id, self.items.len());
                    self.push(TranscriptItem::ToolCall(ToolCallState::from_call(call)));
                }
            }
            SessionUpdate::ToolCallUpdate(update) => self.apply_tool_call_update(update),
            SessionUpdate::Plan(plan) => {
                self.plan = Some(plan);
            }
            SessionUpdate::AvailableCommandsUpdate(update) => {
                self.available_commands = update.available_commands;
            }
            SessionUpdate::CurrentModeUpdate(update) => {
                self.current_mode = Some(update.current_mode_id);
            }
            SessionUpdate::ConfigOptionUpdate(update) => {
                self.config_options = update.config_options;
            }
            SessionUpdate::UsageUpdate(usage) => {
                self.usage = Some(usage);
            }
            SessionUpdate::SessionInfoUpdate(info) => {
                if let agent_client_protocol::schema::MaybeUndefined::Value(title) = info.title {
                    self.title = Some(title);
                }
            }
            // `UserMessageChunk` is ignored on purpose: we add the user's prompts ourselves when sending
            // them, and echoed chunks only matter when replaying a loaded session.
            _ => {}
        }
    }

    /// Marks every tool call still pending or running as failed with `reason`.
    ///
    /// For when the turn ends without the agent reporting on them: cancelled, or the agent went
    /// away. Otherwise they would show a spinner forever.
    pub fn fail_unfinished_tool_calls(&mut self, reason: &str) {
        for entry in &mut self.items {
            if let TranscriptItem::ToolCall(call) = &mut entry.item
                && matches!(
                    call.status,
                    ToolCallStatus::Pending | ToolCallStatus::InProgress
                )
            {
                call.status = ToolCallStatus::Failed;
                call.content
                    .push(ToolCallContent::from(ContentBlock::Text(TextContent::new(
                        reason.to_owned(),
                    ))));
            }
        }
    }

    /// Permission requests carry a tool call update too, and it should show up in the transcript.
    pub fn apply_tool_call_update(&mut self, update: ToolCallUpdate) {
        if let Some(&index) = self.tool_calls.get(&update.tool_call_id) {
            if let Some(TranscriptEntry {
                item: TranscriptItem::ToolCall(existing),
                ..
            }) = self.items.get_mut(index)
            {
                existing.apply(update.fields);
            }
        } else {
            self.tool_calls
                .insert(update.tool_call_id.clone(), self.items.len());
            self.push(TranscriptItem::ToolCall(ToolCallState::from_update(update)));
        }
    }

    /// The agent message currently being streamed, creating one if the last item is something else.
    fn agent_item(&mut self) -> (&mut Vec<ContentBlock>, &mut String) {
        let is_agent = matches!(
            self.items.last(),
            Some(TranscriptEntry {
                item: TranscriptItem::Agent { .. },
                ..
            })
        );
        if !is_agent {
            self.push(TranscriptItem::Agent {
                content: Vec::new(),
                thoughts: String::new(),
            });
        }
        match self.items.last_mut().map(|entry| &mut entry.item) {
            Some(TranscriptItem::Agent { content, thoughts }) => (content, thoughts),
            _ => unreachable!("we just made sure the last item is an agent message"),
        }
    }
}

/// Appends a block, merging consecutive text blocks so they render as one markdown document.
fn push_merged(content: &mut Vec<ContentBlock>, block: ContentBlock) {
    if let (ContentBlock::Text(new), Some(ContentBlock::Text(last))) = (&block, content.last_mut())
    {
        last.text.push_str(&new.text);
    } else {
        content.push(block);
    }
}

impl Transcript {
    /// Plain-text rendering, for logs and headless runs.
    pub fn to_plain_text(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        for entry in &self.items {
            match &entry.item {
                TranscriptItem::User { text } => {
                    writeln!(out, "You: {text}").ok();
                }
                TranscriptItem::Agent { content, thoughts } => {
                    if !thoughts.is_empty() {
                        writeln!(out, "Agent (thinking): {thoughts}").ok();
                    }
                    for block in content {
                        match block {
                            ContentBlock::Text(text) => {
                                writeln!(out, "Agent: {}", text.text).ok();
                            }
                            ContentBlock::Image(image) => {
                                writeln!(out, "Agent: [image {}]", image.mime_type).ok();
                            }
                            _ => {
                                writeln!(out, "Agent: [content]").ok();
                            }
                        }
                    }
                }
                TranscriptItem::ToolCall(call) => {
                    writeln!(out, "[{:?} {:?}] {}", call.kind, call.status, call.title).ok();
                }
                TranscriptItem::Note { text, is_error } => {
                    let prefix = if *is_error { "Error" } else { "Note" };
                    writeln!(out, "{prefix}: {text}").ok();
                }
            }
        }
        out
    }
}

/// Longest tool-call input or output kept by [`Transcript::to_markdown`], in bytes.
///
/// One screenshot or whole-file read would otherwise bury everything else in the dump.
const MAX_TOOL_TEXT_BYTES: usize = 2000;

impl Transcript {
    /// Markdown dump of the whole session, for another agent to read.
    ///
    /// Unlike [`Self::to_plain_text`] this keeps the evidence of what each tool call did: its
    /// arguments, its output, and how long into the session it happened. Tool evidence is cut to
    /// a couple of thousand bytes and images are named rather than embedded, but what the agent
    /// itself said is kept whole.
    pub fn to_markdown(&self) -> String {
        use std::fmt::Write as _;

        let start = self.items.first().map(|entry| entry.created_at);
        let mut out = String::new();
        if let Some(title) = &self.title {
            writeln!(out, "# {title}\n").ok();
        }

        for entry in &self.items {
            let elapsed = start
                .and_then(|start| entry.created_at.duration_since(start).ok())
                .map_or_else(String::new, |elapsed| {
                    format!(" `+{:.1}s`", elapsed.as_secs_f64())
                });

            match &entry.item {
                TranscriptItem::User { text } => {
                    writeln!(out, "## User{elapsed}\n\n{text}\n").ok();
                }

                TranscriptItem::Agent { content, thoughts } => {
                    writeln!(out, "## Agent{elapsed}\n").ok();
                    if !thoughts.is_empty() {
                        writeln!(
                            out,
                            "<thinking>\n{}\n</thinking>\n",
                            truncate_bytes(thoughts, MAX_TOOL_TEXT_BYTES)
                        )
                        .ok();
                    }
                    for block in content {
                        writeln!(out, "{}\n", describe_agent_block(block)).ok();
                    }
                }

                TranscriptItem::ToolCall(call) => {
                    writeln!(
                        out,
                        "## Tool call{elapsed}: {} ({:?}, {:?})\n",
                        call.title, call.kind, call.status
                    )
                    .ok();
                    if let Some(raw_input) = &call.raw_input {
                        writeln!(out, "Input:\n\n```json\n{}\n```\n", json_excerpt(raw_input)).ok();
                    }
                    for content in &call.content {
                        writeln!(out, "{}\n", describe_tool_call_content(content)).ok();
                    }
                    if let Some(raw_output) = &call.raw_output {
                        writeln!(
                            out,
                            "Output:\n\n```json\n{}\n```\n",
                            json_excerpt(raw_output)
                        )
                        .ok();
                    }
                }

                TranscriptItem::Note { text, is_error } => {
                    let prefix = if *is_error { "Error" } else { "Note" };
                    writeln!(out, "## {prefix}{elapsed}\n\n{text}\n").ok();
                }
            }
        }
        out
    }
}

/// One block of what the agent itself said, kept whole.
///
/// The agent's own words are what the reading agent is here to judge, and a final answer can
/// run well past what a tool call is allowed, so nothing is cut here.
fn describe_agent_block(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text.clone(),
        _ => describe_content_block(block),
    }
}

fn describe_content_block(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => truncate_bytes(&text.text, MAX_TOOL_TEXT_BYTES),
        ContentBlock::Image(image) => format!("`[image {}]`", image.mime_type),
        ContentBlock::Audio(audio) => format!("`[audio {}]`", audio.mime_type),
        _ => "`[content]`".to_owned(),
    }
}

fn describe_tool_call_content(content: &ToolCallContent) -> String {
    match content {
        ToolCallContent::Content(content) => describe_content_block(&content.content),
        // Naming the file says an edit happened but not what it did, and an edit tool often
        // carries no `raw_output` to fall back on, which leaves the reading agent unable to
        // tell a one-line fix from a rewrite.
        ToolCallContent::Diff(diff) => {
            use std::fmt::Write as _;

            let mut out = format!("`[diff of {}]`\n", diff.path.display());
            if let Some(old_text) = &diff.old_text {
                write!(
                    out,
                    "\nBefore:\n\n```\n{}\n```\n",
                    truncate_bytes(old_text, MAX_TOOL_TEXT_BYTES)
                )
                .ok();
            }
            write!(
                out,
                "\nAfter:\n\n```\n{}\n```",
                truncate_bytes(&diff.new_text, MAX_TOOL_TEXT_BYTES)
            )
            .ok();
            out
        }
        ToolCallContent::Terminal(terminal) => format!("`[terminal {}]`", terminal.terminal_id.0),
        _ => "`[content]`".to_owned(),
    }
}

/// Pretty-printed JSON, cut to [`MAX_TOOL_TEXT_BYTES`].
fn json_excerpt(value: &serde_json::Value) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    truncate_bytes(&text, MAX_TOOL_TEXT_BYTES)
}

/// `text`, cut to `max_bytes` with a note saying how much was left out.
///
/// The cut lands on the nearest character boundary at or below `max_bytes`, so a multi-byte
/// character is dropped whole rather than split.
fn truncate_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let kept = &text[..text.floor_char_boundary(max_bytes)];
    format!("{kept}…\n[truncated: {} bytes in all]", text.len())
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::v1::{
        ConfigOptionUpdate, ContentChunk, Diff, SessionConfigSelectGroup,
        SessionConfigSelectOption, ToolCall,
    };

    use super::*;

    fn transcript(config_options: Vec<SessionConfigOption>) -> Transcript {
        let mut transcript = Transcript::default();
        transcript.apply(SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(
            config_options,
        )));
        transcript
    }

    fn model_selector(current: &'static str) -> SessionConfigOption {
        SessionConfigOption::select(
            "model",
            "Model",
            current,
            vec![
                SessionConfigSelectOption::new("opus", "Opus 4.6"),
                SessionConfigSelectOption::new("sonnet", "Sonnet 4.5"),
            ],
        )
        .category(SessionConfigOptionCategory::Model)
    }

    #[test]
    fn names_the_selected_model() {
        let options = vec![
            SessionConfigOption::boolean("brave_mode", "Brave Mode", false),
            model_selector("sonnet"),
        ];
        assert_eq!(transcript(options).current_model(), Some("Sonnet 4.5"));
    }

    #[test]
    fn finds_the_selected_model_inside_a_group() {
        let group = SessionConfigSelectGroup::new(
            "anthropic",
            "Anthropic",
            vec![SessionConfigSelectOption::new("opus", "Opus 4.6")],
        );
        let options = vec![
            SessionConfigOption::select("model", "Model", "opus", vec![group])
                .category(SessionConfigOptionCategory::Model),
        ];
        assert_eq!(transcript(options).current_model(), Some("Opus 4.6"));
    }

    #[test]
    fn falls_back_to_the_raw_value_of_a_model_that_is_not_offered() {
        assert_eq!(
            transcript(vec![model_selector("haiku")]).current_model(),
            Some("haiku")
        );
    }

    #[test]
    fn has_no_model_without_a_model_selector() {
        assert_eq!(Transcript::default().current_model(), None);
        let modes = vec![
            SessionConfigOption::select(
                "mode",
                "Mode",
                "normal",
                vec![SessionConfigSelectOption::new("normal", "Normal")],
            )
            .category(SessionConfigOptionCategory::Mode),
        ];
        assert_eq!(transcript(modes).current_model(), None);
    }

    #[test]
    fn the_markdown_dump_keeps_the_evidence_of_a_tool_call() {
        let mut transcript = Transcript::default();
        transcript.push_user("plot the joint angles".to_owned());
        transcript.apply(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new("Looking at the recording.".to_owned())),
        )));
        transcript.apply(SessionUpdate::ToolCall(
            ToolCall::new("call-1", "Set the time cursor")
                .status(ToolCallStatus::Failed)
                .raw_input(serde_json::json!({ "time": 12 }))
                .raw_output(serde_json::json!("no such timeline")),
        ));
        transcript.push_note("the agent gave up", true);

        let markdown = transcript.to_markdown();
        assert!(markdown.contains("## User"));
        assert!(markdown.contains("plot the joint angles"));
        assert!(markdown.contains("Looking at the recording."));
        assert!(markdown.contains("Set the time cursor"));
        assert!(markdown.contains("Failed"));
        // Both the arguments and the result are there: the two things `to_plain_text` drops,
        // and the only way a reviewer can see what a call actually did.
        assert!(markdown.contains("\"time\": 12"));
        assert!(markdown.contains("no such timeline"));
        assert!(markdown.contains("## Error"));
        assert!(markdown.contains("the agent gave up"));
    }

    /// The cut exists to stop one screenshot burying the dump, not to clip the answer the
    /// reading agent is there to judge.
    #[test]
    fn a_long_agent_answer_survives_the_dump_whole() {
        let mut transcript = Transcript::default();
        let long = "word ".repeat(MAX_TOOL_TEXT_BYTES);
        transcript.apply(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new(long.clone())),
        )));

        let markdown = transcript.to_markdown();
        assert!(markdown.contains(long.trim_end()));
        assert!(!markdown.contains("[truncated:"));
    }

    /// A tool call that edits a file often carries no `raw_output`, so the diff is the only
    /// record of what changed.
    #[test]
    fn a_diff_reports_both_sides_not_just_the_path() {
        let mut transcript = Transcript::default();
        transcript.apply(SessionUpdate::ToolCall(
            ToolCall::new("call-1", "Edit preamble.rs").content(vec![ToolCallContent::from(
                Diff::new("preamble.rs", "the old line").old_text("the old line"),
            )]),
        ));

        let markdown = transcript.to_markdown();
        assert!(markdown.contains("preamble.rs"), "{markdown}");
        assert!(markdown.contains("the old line"), "{markdown}");
    }

    #[test]
    fn long_tool_output_is_cut_without_splitting_a_character() {
        let mut transcript = Transcript::default();
        let long = "å".repeat(MAX_TOOL_TEXT_BYTES + 1);
        transcript.apply(SessionUpdate::ToolCall(
            ToolCall::new("call-1", "Read a file").content(vec![ToolCallContent::from(
                ContentBlock::Text(TextContent::new(long)),
            )]),
        ));

        let markdown = transcript.to_markdown();
        // The limit is a byte count, but it lands on a character boundary, so every `å` that
        // survives is whole and the dump is still valid UTF-8.
        assert_eq!(
            markdown.matches('å').count(),
            MAX_TOOL_TEXT_BYTES / 'å'.len_utf8()
        );
        assert!(markdown.contains("[truncated:"));
    }

    /// A limit landing mid-character is what `floor_char_boundary` is for: the cut moves down to
    /// the boundary instead of panicking on a byte index inside a character.
    #[test]
    fn a_limit_inside_a_character_cuts_below_it() {
        let text = "å".repeat(4);
        assert_eq!(truncate_bytes(&text, 5).matches('å').count(), 2);
    }

    #[test]
    fn text_of_exactly_the_limit_is_left_alone() {
        let text = "x".repeat(MAX_TOOL_TEXT_BYTES);
        assert_eq!(truncate_bytes(&text, MAX_TOOL_TEXT_BYTES), text);
        assert!(truncate_bytes("", MAX_TOOL_TEXT_BYTES).is_empty());
    }

    #[test]
    fn an_empty_transcript_dumps_to_nothing() {
        assert!(Transcript::default().to_markdown().is_empty());
    }
}
