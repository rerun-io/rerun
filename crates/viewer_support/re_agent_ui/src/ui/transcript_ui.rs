use egui::RichText;
use re_agent::acp::schema::v1::{Plan, PlanEntryStatus, ToolCallStatus};
use re_format::{DateVisibility, TimestampFormat};
use re_ui::{ButtonExt as _, UiExt as _, icons};
use web_time::SystemTime;

use super::conversation_list::status_dot_ui;
use super::tool_call_ui::{content_block_ui, tool_call_ui};
use re_agent::{
    AgentSession, McpStartupFailure, Prompt, PromptImage, TranscriptEntry, TranscriptItem,
    message_text,
};

/// Each ended turn with an agent answer gets a footer that offers to copy that answer.
///
/// In an ended turn, consecutive tool calls fold into one row.
///
/// The copied text is the last agent message of the turn with non-blank text, not the whole turn.
/// The last turn counts as ended only when no turn is in progress.
pub fn transcript_ui(ui: &mut egui::Ui, session: &AgentSession, show_thoughts: bool) {
    let transcript = session.transcript();
    ui.spacing_mut().item_spacing.y = 10.0;

    let transcript_id = ui.make_persistent_id("transcript");
    let items = &transcript.items;
    let last_user_index = items
        .iter()
        .rposition(|entry| matches!(entry.item, TranscriptItem::User(_)));

    let mut turn = TurnState::default();
    let mut index = 0;
    while let Some(entry) = items.get(index) {
        turn.observe(ui, entry);

        let turn_ended = last_user_index.is_some_and(|last_user| index < last_user)
            || !session.turn_in_progress();
        let folded_calls = if turn_ended {
            tool_call_run_len(items, index)
        } else {
            0
        };
        let next_index = index + folded_calls.max(1);

        let scope = ui.push_id(index, |ui| {
            if 2 <= folded_calls {
                let run = items.get(index..next_index).unwrap_or_default();
                Some(tool_call_group_ui(ui, session, run, transcript_id))
            } else {
                entry_ui(ui, session, entry, show_thoughts, transcript_id)
            }
        });
        let rect = scope.inner.unwrap_or(scope.response.rect);
        if ui.rect_contains_pointer(rect) {
            timestamp_overlay(ui, rect, entry);
        }

        let turn_ends_here = match items.get(next_index) {
            Some(next) => matches!(next.item, TranscriptItem::User(_)),
            None => !session.turn_in_progress(),
        };
        if turn_ends_here
            && let (Some(turn_top), Some((answer, answered_at))) = (turn.top, &turn.last_answer)
        {
            ui.push_id(("turn_footer", index), |ui| {
                turn_footer_ui(ui, turn_top, answer, *answered_at);
            });
        }

        index = next_index;
    }

    // A running tool call already shows its own indicator.
    let tool_call_running = items.iter().any(|entry| {
        matches!(
            &entry.item,
            TranscriptItem::ToolCall(call)
                if matches!(call.status, ToolCallStatus::Pending | ToolCallStatus::InProgress)
        )
    });
    if session.turn_in_progress() && !session.is_waiting_on_user() && !tool_call_running {
        ui.inline_loading_indicator("The agent is working");
    }
}

/// What the turn footer needs to know about the turn shown so far.
#[derive(Default)]
struct TurnState {
    /// Where the turn's user message starts, in the same space as `ui.cursor()`.
    top: Option<f32>,

    /// The last agent message with non-blank text, and when it was written.
    last_answer: Option<(String, SystemTime)>,
}

impl TurnState {
    /// Call for each entry before showing it.
    fn observe(&mut self, ui: &egui::Ui, entry: &TranscriptEntry) {
        match &entry.item {
            TranscriptItem::User(_) => {
                self.top = Some(ui.cursor().top());
                self.last_answer = None;
            }
            TranscriptItem::Agent { content, .. } => {
                let text = message_text(content);
                if !text.trim().is_empty() {
                    self.last_answer = Some((text, entry.created_at));
                }
            }
            TranscriptItem::McpStartupFailure(_)
            | TranscriptItem::Note { .. }
            | TranscriptItem::ToolCall(_) => {}
        }
    }
}

/// How many tool calls follow each other from `index` on.
fn tool_call_run_len(items: &[TranscriptEntry], index: usize) -> usize {
    items
        .iter()
        .skip(index)
        .take_while(|entry| matches!(entry.item, TranscriptItem::ToolCall(_)))
        .count()
}

/// One transcript entry. Returns the rect to show its hover timestamp for, if it is not the
/// whole entry.
fn entry_ui(
    ui: &mut egui::Ui,
    session: &AgentSession,
    entry: &TranscriptEntry,
    show_thoughts: bool,
    transcript_id: egui::Id,
) -> Option<egui::Rect> {
    match &entry.item {
        TranscriptItem::User(prompt) => {
            Some(user_message_ui(ui, prompt, ui.make_persistent_id("user")))
        }
        TranscriptItem::Agent { content, thoughts } => {
            if show_thoughts && !thoughts.is_empty() {
                thoughts_ui(ui, thoughts);
            }
            for (block_index, block) in content.iter().enumerate() {
                content_block_ui(ui, block, ui.make_persistent_id(block_index));
            }
            None
        }
        TranscriptItem::ToolCall(call) => {
            tool_call_ui(ui, session, call, transcript_id.with(&call.id.0));
            None
        }
        TranscriptItem::McpStartupFailure(failure) => {
            mcp_startup_failure_ui(ui, failure);
            None
        }
        TranscriptItem::Note { text, is_error } => {
            if *is_error {
                ui.error_label(text.clone());
            } else {
                ui.weak(text);
            }
            None
        }
    }
}

/// Consecutive tool calls of an ended turn, folded into one collapsed header.
///
/// The header shows a red dot only if the last call failed: earlier failures the agent recovered from are noise.
/// Returns the rect of the header, which shows the time of the first call on hover.
fn tool_call_group_ui(
    ui: &mut egui::Ui,
    session: &AgentSession,
    run: &[TranscriptEntry],
    transcript_id: egui::Id,
) -> egui::Rect {
    let calls: Vec<_> = run
        .iter()
        .filter_map(|entry| match &entry.item {
            TranscriptItem::ToolCall(call) => Some((entry, call)),
            _ => None,
        })
        .collect();
    let last_failed = calls
        .last()
        .is_some_and(|(_, call)| call.status == ToolCallStatus::Failed);

    let tokens = ui.tokens();
    // Keyed by its first call, not its position, so it never inherits another group's open state.
    let first_call_id = calls.first().map(|(_, call)| &call.id.0);
    let mut label_clicked = false;
    let mut header = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        transcript_id.with(("tool_call_group", first_call_id)),
        false,
    )
    .show_header(ui, |ui| {
        if last_failed {
            status_dot_ui(ui, tokens.error_fg_color);
        }
        label_clicked = ui
            .add(
                egui::Label::new(
                    RichText::new(format!("Used {} tools", calls.len())).color(tokens.text_subdued),
                )
                .selectable(false)
                .sense(egui::Sense::click()),
            )
            .clicked();
    });
    if label_clicked {
        header.toggle();
    }
    let (_, header_response, _) = header.body(|ui| {
        for (entry, call) in &calls {
            let rect = ui
                .scope(|ui| {
                    tool_call_ui(ui, session, call, transcript_id.with(&call.id.0));
                })
                .response
                .rect;
            if ui.rect_contains_pointer(rect) {
                timestamp_overlay(ui, rect, entry);
            }
        }
    });
    header_response.response.rect
}

/// The copy button and the answer time show only while the pointer is over the turn, from `turn_top` down to the footer itself.
///
/// `turn_top` is a y position in the same space as `ui.cursor()`.
/// The footer takes its space even when the button is hidden, so the transcript does not shift on hover.
fn turn_footer_ui(ui: &mut egui::Ui, turn_top: f32, answer: &str, answered_at: SystemTime) {
    let turn_bottom = ui.cursor().top() + re_ui::Size::Small.height();
    let turn_rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), turn_top..=turn_bottom);
    let hovered = ui.rect_contains_pointer(turn_rect);
    ui.horizontal(|ui| {
        if ui
            .add_visible(
                hovered,
                egui::Button::icon(icons::COPY, "Copy response")
                    .ghost()
                    .size_small(),
            )
            .on_hover_text("Copy response")
            .clicked()
        {
            ui.copy_text(answer.to_owned());
        }
        if hovered && let Some(answered_at) = to_jiff(answered_at) {
            re_ui::time::short_duration_ui(
                ui,
                answered_at,
                TimestampFormat::local_timezone()
                    .with_short(true)
                    .with_date_visibility(DateVisibility::ShowDate),
                egui::Ui::weak,
            );
        }
    });
}

/// Paints the time an entry was created.
///
/// Left of a user message bubble when it fits there, else in the gap above the bubble.
/// Other entries get it at the right edge of the panel: vertically centered on a one-line entry,
/// else in the gap above it so it covers no text.
fn timestamp_overlay(ui: &egui::Ui, entry_rect: egui::Rect, entry: &TranscriptEntry) {
    let tokens = ui.tokens();
    let font_id = egui::TextStyle::Small.resolve(ui.style());
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(
        format_timestamp(entry.created_at),
        font_id,
        tokens.text_subdued,
    );
    let size = galley.size();
    let padding = 2.0;

    let is_user = matches!(entry.item, TranscriptItem::User(_));
    if is_user {
        let right = entry_rect.left() - 2.0 * padding;
        if ui.max_rect().left() <= right - size.x {
            let first_line_height = ui.text_style_height(&egui::TextStyle::Body);
            let top = (entry_rect.top() + USER_FRAME_MARGIN + 0.5 * (first_line_height - size.y))
                .max(ui.clip_rect().top() + padding);
            painter.galley(egui::pos2(right - size.x, top), galley, tokens.text_subdued);
            return;
        }
    }

    let entry_background = if is_user {
        tokens.form_field_bg_color
    } else {
        ui.visuals().panel_fill
    };
    // Half a line of slack: a wrapped entry is at least a full line taller.
    let one_line_height = 1.5 * ui.text_style_height(&egui::TextStyle::Body);
    // A user bubble hugs its text, so only the gap above it is free.
    let top = if !is_user && entry_rect.height() <= one_line_height {
        entry_rect.center().y - 0.5 * size.y
    } else {
        // Keep it visible when the gap above is scrolled away, at the cost of covering text.
        (entry_rect.top() - size.y - padding).max(ui.clip_rect().top() + padding)
    };
    let background = if entry_rect.top() <= top {
        entry_background
    } else {
        ui.visuals().panel_fill
    };

    let right = if is_user {
        entry_rect.right()
    } else {
        ui.max_rect().right() - USER_FRAME_MARGIN
    };
    let text_rect = egui::Rect::from_min_size(egui::pos2(right - size.x, top), size);
    painter.rect_filled(text_rect.expand(padding), padding, background);
    painter.galley(text_rect.min, galley, tokens.text_subdued);
}

fn mcp_startup_failure_ui(ui: &mut egui::Ui, failure: &McpStartupFailure) {
    let McpStartupFailure {
        server_name,
        error,
        requested_command: _,
        requested_args: _,
        hint,
    } = failure;
    let title = format!("MCP server {server_name:?} failed to start");
    re_ui::alert::Alert::error().show_with_title(ui, title, |ui| {
        if let Some(command) = failure.requested_command_line() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Requested by Rerun:");
                ui.add(egui::Label::new(RichText::new(command).monospace()).selectable(true));
            });
        }
        ui.add(egui::Label::new(error).selectable(true));
        if let Some(hint) = hint {
            ui.label(hint);
        }
    });
}

/// `jiff` only converts from `std::time::SystemTime`, which is a different type on the web.
fn to_jiff(time: SystemTime) -> Option<jiff::Timestamp> {
    let since_epoch = time.duration_since(web_time::UNIX_EPOCH).ok()?;
    jiff::Timestamp::from_duration(since_epoch.try_into().ok()?).ok()
}

/// Local wall-clock time, e.g. `14:03:12`, with the date in front when it isn't today.
fn format_timestamp(time: SystemTime) -> String {
    let Some(timestamp) = to_jiff(time) else {
        return "unknown time".to_owned();
    };
    let zoned = timestamp.to_zoned(jiff::tz::TimeZone::system());
    let format = if zoned.date() == jiff::Zoned::now().date() {
        "%H:%M:%S"
    } else {
        "%Y-%m-%d %H:%M:%S"
    };
    zoned.strftime(format).to_string()
}

/// Inner margin of the frame around a user message, in points.
const USER_FRAME_MARGIN: f32 = 8.0;

/// A user message is at most this fraction of the transcript width,
/// so it reads as a reply bubble rather than a full-width block.
const USER_MESSAGE_MAX_WIDTH_FRACTION: f32 = 0.8;

/// A bubble on the right, as wide as its content. Returns the bubble's rect.
fn user_message_ui(ui: &mut egui::Ui, prompt: &Prompt, id: egui::Id) -> egui::Rect {
    let Prompt { text, images } = prompt;
    let tokens = ui.tokens();
    let frame = egui::Frame::new()
        .fill(tokens.form_field_bg_color)
        .stroke(tokens.control_stroke)
        .corner_radius(6)
        .inner_margin(USER_FRAME_MARGIN);
    let frame_width = frame.total_margin().sum().x;
    let available = ui.available_rect_before_wrap();
    let max_width = (USER_MESSAGE_MAX_WIDTH_FRACTION * available.width() - frame_width).max(0.0);

    // Measure first, so the bubble can hug its content from the right edge.
    let text_width = if text.is_empty() {
        0.0
    } else {
        egui::WidgetText::from(text)
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Wrap),
                max_width,
                egui::TextStyle::Body,
            )
            .size()
            .x
    };

    // Expand the bubble up to `max_width` if the prompt contains images wider than the text.
    let content_width = images
        .iter()
        .map(|image| image.resolution[0] as f32)
        .fold(text_width, f32::max)
        .min(max_width)
        .ceil();

    let bubble_width = content_width + frame_width;
    let bubble_rect = egui::Rect::from_min_max(
        egui::pos2(available.right() - bubble_width, available.top()),
        available.max,
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(bubble_rect), |ui| {
        frame.show(ui, |ui| {
            ui.set_width(content_width);
            if !text.is_empty() {
                ui.add(egui::Label::new(text).selectable(true));
            }
            for (index, image) in images.iter().enumerate() {
                attached_image_ui(ui, image, id.with(index));
            }
        });
    })
    .response
    .rect
}

/// One image the user attached, at its own size up to the width of the message.
fn attached_image_ui(ui: &mut egui::Ui, image: &PromptImage, id: egui::Id) {
    // The bytes are already encoded, so egui's loaders decode and cache them: the URI only has
    // to be stable across frames and unique to this image.
    let extension = image.mime_type.rsplit('/').next().unwrap_or("png");
    let uri = format!("bytes://{}.{extension}", id.short_debug_format());
    // Shown at its own size, shrunk to fit but never blown up: a pasted screenshot is read, and
    // upscaling only softens it.
    let [width, height] = image.resolution;
    let natural = egui::vec2(width as f32, height as f32); // One point per pixel
    let max = egui::vec2(natural.x.min(ui.available_width()), natural.y);
    ui.add(
        egui::Image::from_bytes(uri, image.bytes.clone())
            .max_size(max)
            .corner_radius(4),
    );
}

fn thoughts_ui(ui: &mut egui::Ui, thoughts: &str) {
    let tokens = ui.tokens();
    egui::CollapsingHeader::new(
        RichText::new("Thinking")
            .italics()
            .color(tokens.text_subdued),
    )
    .id_salt("thoughts")
    .default_open(false)
    .show(ui, |ui| {
        ui.label(RichText::new(thoughts).italics().color(tokens.text_subdued));
    });
}

/// One collapsed line with the progress, which expands into the whole plan.
pub fn plan_ui(ui: &mut egui::Ui, plan: &Plan) {
    if plan.entries.is_empty() {
        return;
    }
    let done = plan
        .entries
        .iter()
        .filter(|entry| entry.status == PlanEntryStatus::Completed)
        .count();
    let title = format!("Plan ({done}/{})", plan.entries.len());
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .id_salt("plan")
        .default_open(false)
        .show(ui, |ui| {
            let tokens = ui.tokens();
            for entry in &plan.entries {
                let (icon, color) = match entry.status {
                    PlanEntryStatus::InProgress => (&icons::PLAN_IN_PROGRESS, tokens.text_strong),
                    PlanEntryStatus::Completed => {
                        (&icons::PLAN_COMPLETED, tokens.success_text_color)
                    }
                    _ => (&icons::PLAN_PENDING, tokens.text_subdued),
                };
                ui.horizontal(|ui| {
                    ui.small_icon(icon, Some(color));
                    let mut text = RichText::new(&entry.content);
                    if entry.status == PlanEntryStatus::Completed {
                        text = text.strikethrough().color(tokens.text_subdued);
                    }
                    ui.label(text);
                });
            }
        });
}
