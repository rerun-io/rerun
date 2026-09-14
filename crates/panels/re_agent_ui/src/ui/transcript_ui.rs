use egui::RichText;
use re_agent::acp::schema::v1::{Plan, PlanEntryStatus};
use re_ui::{UiExt as _, icons};

use super::tool_call_ui::{content_block_ui, tool_call_ui};
use re_agent::{Transcript, TranscriptItem};

pub fn transcript_ui(ui: &mut egui::Ui, transcript: &Transcript, show_thoughts: bool) {
    ui.spacing_mut().item_spacing.y = 10.0;

    for (index, entry) in transcript.items.iter().enumerate() {
        let response = ui
            .push_id(index, |ui| match &entry.item {
                TranscriptItem::User { text } => user_message_ui(ui, text),
                TranscriptItem::Agent { content, thoughts } => {
                    if show_thoughts && !thoughts.is_empty() {
                        thoughts_ui(ui, thoughts);
                    }
                    for (block_index, block) in content.iter().enumerate() {
                        content_block_ui(ui, block, ui.id().with(block_index));
                    }
                }
                TranscriptItem::ToolCall(call) => tool_call_ui(ui, call),
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
    }
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

fn user_message_ui(ui: &mut egui::Ui, text: &str) {
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
            ui.add(egui::Label::new(text).selectable(true));
        });
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
