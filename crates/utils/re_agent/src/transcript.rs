use std::collections::HashMap;

use agent_client_protocol::schema::v1::{
    AvailableCommand, ContentBlock, Plan, SessionModeId, SessionUpdate, TextContent, ToolCall,
    ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, UsageUpdate,
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
