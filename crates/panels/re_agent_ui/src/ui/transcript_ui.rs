use egui::RichText;
use re_agent::acp::schema::v1::{Plan, PlanEntryStatus};
use re_ui::{ReButton, UiExt as _, icons};

use super::tool_call_ui::{content_block_ui, tool_call_ui};
use re_agent::{
    AgentSession, McpStartupFailure, Prompt, PromptImage, TranscriptEntry, TranscriptItem,
    message_text,
};

/// Each ended turn with an agent answer gets a footer that offers to copy that answer.
///
/// The copied text is the last agent message of the turn with non-blank text, not the whole turn.
/// The last turn counts as ended only when no turn is in progress.
pub fn transcript_ui(ui: &mut egui::Ui, session: &AgentSession, show_thoughts: bool) {
    let transcript = session.transcript();
    ui.spacing_mut().item_spacing.y = 10.0;

    let mut turn_top = None;
    let mut last_answer: Option<String> = None;
    for (index, entry) in transcript.items.iter().enumerate() {
        match &entry.item {
            TranscriptItem::User(_) => {
                turn_top = Some(ui.cursor().top());
                last_answer = None;
            }
            TranscriptItem::Agent { content, .. } => {
                let text = message_text(content);
                if !text.trim().is_empty() {
                    last_answer = Some(text);
                }
            }
            TranscriptItem::McpStartupFailure(_)
            | TranscriptItem::Note { .. }
            | TranscriptItem::ToolCall(_) => {}
        }

        let rect = ui
            .push_id(index, |ui| match &entry.item {
                TranscriptItem::User(prompt) => {
                    user_message_ui(ui, prompt, ui.make_persistent_id("user"));
                }
                TranscriptItem::Agent { content, thoughts } => {
                    if show_thoughts && !thoughts.is_empty() {
                        thoughts_ui(ui, thoughts);
                    }
                    for (block_index, block) in content.iter().enumerate() {
                        content_block_ui(ui, block, ui.make_persistent_id(block_index));
                    }
                }
                TranscriptItem::ToolCall(call) => tool_call_ui(ui, call),
                TranscriptItem::McpStartupFailure(failure) => {
                    mcp_startup_failure_ui(ui, failure);
                }
                TranscriptItem::Note { text, is_error } => {
                    if *is_error {
                        ui.error_label(text.clone());
                    } else {
                        ui.weak(text);
                    }
                }
            })
            .response
            .rect;
        if ui.rect_contains_pointer(rect) {
            timestamp_overlay(ui, rect, entry);
        }

        let turn_ends_here = match transcript.items.get(index + 1) {
            Some(next) => matches!(next.item, TranscriptItem::User(_)),
            None => !session.turn_in_progress(),
        };
        if turn_ends_here && let (Some(turn_top), Some(answer)) = (turn_top, &last_answer) {
            ui.push_id(("turn_footer", index), |ui| {
                turn_footer_ui(ui, turn_top, answer);
            });
        }
    }
}

/// The copy button shows only while the pointer is over the turn, from `turn_top` down to the footer itself.
///
/// `turn_top` is a y position in the same space as `ui.cursor()`.
/// The footer takes its space even when the button is hidden, so the transcript does not shift on hover.
fn turn_footer_ui(ui: &mut egui::Ui, turn_top: f32, answer: &str) {
    let turn_bottom = ui.cursor().top() + re_ui::Size::Small.height();
    let turn_rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), turn_top..=turn_bottom);
    let hovered = ui.rect_contains_pointer(turn_rect);
    ui.horizontal(|ui| {
        if ui
            .add_visible(
                hovered,
                ReButton::icon(icons::COPY, "Copy response").ghost().small(),
            )
            .on_hover_text("Copy response")
            .clicked()
        {
            ui.copy_text(answer.to_owned());
        }
    });
}

/// Paints the time an entry was created at the right edge of the panel:
/// vertically centered on a one-line entry, else in the gap above it so it covers no text.
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

    let (entry_background, frame_margin) = match &entry.item {
        TranscriptItem::User(_) => (tokens.form_field_bg_color, USER_FRAME_MARGIN),
        _ => (ui.visuals().panel_fill, 0.0),
    };
    // Half a line of slack: a wrapped entry is at least a full line taller.
    let one_line_height = 1.5 * ui.text_style_height(&egui::TextStyle::Body) + 2.0 * frame_margin;
    let top = if entry_rect.height() <= one_line_height {
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

    let right = ui.max_rect().right() - USER_FRAME_MARGIN;
    let text_rect = egui::Rect::from_min_size(egui::pos2(right - size.x, top), size);
    painter.rect_filled(text_rect.expand(padding), padding, background);
    painter.galley(text_rect.min, galley, tokens.text_subdued);
}

fn mcp_startup_failure_ui(ui: &mut egui::Ui, failure: &McpStartupFailure) {
    let mut lines = vec![format!(
        "MCP server `{}` failed to start.",
        failure.server_name
    )];
    if let Some(command) = failure.requested_command_line() {
        lines.push(format!("Requested by Rerun: `{command}`"));
    }
    lines.push(failure.error.clone());
    if let Some(hint) = &failure.hint {
        lines.push(hint.clone());
    }
    ui.error_label(lines.join("\n"));
}

/// Local wall-clock time, e.g. `14:03:12`, with the date in front when it isn't today.
fn format_timestamp(time: std::time::SystemTime) -> String {
    let Ok(timestamp) = jiff::Timestamp::try_from(time) else {
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

fn user_message_ui(ui: &mut egui::Ui, prompt: &Prompt, id: egui::Id) {
    let Prompt { text, images } = prompt;
    let tokens = ui.tokens();
    egui::Frame::new()
        .fill(tokens.form_field_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            tokens.widget_noninteractive_bg_stroke,
        ))
        .corner_radius(6)
        .inner_margin(USER_FRAME_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !text.is_empty() {
                ui.add(egui::Label::new(text).selectable(true));
            }
            for (index, image) in images.iter().enumerate() {
                attached_image_ui(ui, image, id.with(index));
            }
        });
}

/// One image the user attached, at its own size up to the width of the message.
fn attached_image_ui(ui: &mut egui::Ui, image: &PromptImage, id: egui::Id) {
    // The bytes are already encoded, so egui's loaders decode and cache them: the URI only has
    // to be stable across frames and unique to this image.
    let extension = image.mime_type.rsplit('/').next().unwrap_or("png");
    let uri = format!("bytes://{}.{extension}", id.short_debug_format());
    // Shown at its own size, shrunk to fit but never blown up: a pasted screenshot is read, and
    // upscaling only softens it.
    let [width, height] = image.size;
    let natural = egui::vec2(width as f32, height as f32);
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
        .default_open(true)
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
