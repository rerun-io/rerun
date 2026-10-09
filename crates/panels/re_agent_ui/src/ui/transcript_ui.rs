use egui::RichText;
use re_agent::acp::schema::v1::{Plan, PlanEntryStatus};
use re_ui::{ReButton, UiExt as _, icons};

use super::tool_call_ui::{content_block_ui, tool_call_ui};
use re_agent::{
    AgentSession, McpStartupFailure, Prompt, PromptImage, TranscriptItem, message_text,
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

        let response = ui
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
            .response;
        response.on_hover_text(format_timestamp(entry.created_at));

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

/// Local wall-clock time, with the date, e.g. `2026-09-08 14:03:12`.
fn format_timestamp(time: std::time::SystemTime) -> String {
    jiff::Timestamp::try_from(time)
        .map(|timestamp| {
            timestamp
                .to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| "unknown time".to_owned())
}

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
        .inner_margin(8)
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
