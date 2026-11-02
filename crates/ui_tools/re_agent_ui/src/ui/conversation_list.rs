//! The conversation list: one row per conversation, with an icon for its status.

use re_ui::list_item::{CustomContent, ListItemContentButtonsExt as _};
use re_ui::{UiExt as _, icons};
use web_time::SystemTime;

use super::agent_panel::{ConversationId, NEW_CONVERSATION_SHORTCUT, PanelAction};

/// What a conversation needs from the user, for its row in the conversation list.
#[derive(Clone, PartialEq, Eq)]
pub enum ConversationStatus {
    /// Waiting on the user, e.g. for a permission or an answer, or has messages the user has
    /// not seen.
    Notification { needs_input: bool },

    /// The agent stopped with an error.
    Errored { message: String },

    /// In the middle of a turn.
    Working,

    /// Nothing the user has not seen.
    NothingNew,
}

impl ConversationStatus {
    /// Where it goes in the list: what needs the user most comes first.
    pub fn rank(&self) -> u8 {
        match self {
            Self::Notification { needs_input: true } => 0,
            Self::Notification { needs_input: false } => 1,
            Self::Errored { .. } => 2,
            Self::Working => 3,
            Self::NothingNew => 4,
        }
    }
}

/// One row of the conversation list, gathered before the list is shown.
pub struct ConversationRow {
    /// Which conversation the row opens or closes.
    pub id: ConversationId,

    /// One line, truncated to fit.
    pub title: String,

    /// Shown as an icon at the right end.
    pub status: ConversationStatus,

    /// When the user last sent a message, for sorting.
    pub last_prompt_at: SystemTime,
}

/// Returns what the user picked: a conversation to open, or one to close.
pub fn conversation_list_ui(ui: &mut egui::Ui, rows: &[ConversationRow]) -> Option<PanelAction> {
    let mut action = None;
    egui::containers::Sides::new().show(
        ui,
        |ui| ui.weak("Conversations"),
        |ui| {
            let shortcut = re_ui::format_shortcut(ui.ctx(), &NEW_CONVERSATION_SHORTCUT);
            if ui
                .small_icon_button(&icons::ADD, "New conversation")
                .on_hover_text(format!("New conversation ({shortcut})"))
                .clicked()
            {
                action = Some(PanelAction::NewConversation);
            }
        },
    );

    egui::ScrollArea::vertical()
        .id_salt("conversation_list")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            re_ui::list_item::list_item_scope(ui, "conversations", |ui| {
                for row in rows {
                    // Rows move when their status changes, so widget state must follow the
                    // conversation, not the position.
                    ui.push_id(row.id, |ui| {
                        let mut close = false;
                        let content = CustomContent::new(|ui, context| {
                            context.response.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Button, true, &row.title)
                            });
                            conversation_row_ui(ui, row);
                        })
                        .with_always_show_buttons(false)
                        .action_button(
                            &icons::CLOSE,
                            "Close conversation",
                            || close = true,
                        );
                        let response = ui.list_item().show_flat(ui, content);
                        if close {
                            action = Some(PanelAction::Close(row.id));
                        } else if response.clicked() {
                            action = Some(PanelAction::Open(row.id));
                        }
                    });
                }
            });
        });
    ui.separator();
    action
}

/// The title of a conversation, and an icon for its status at the right end.
fn conversation_row_ui(ui: &mut egui::Ui, row: &ConversationRow) {
    let ConversationRow {
        id: _,
        title,
        status,
        last_prompt_at: _,
    } = row;
    egui::containers::Sides::new()
        .height(ui.max_rect().height())
        .shrink_left()
        .truncate()
        .show(
            ui,
            |ui| {
                ui.add(egui::Label::new(title.as_str()).selectable(false));
            },
            |ui| {
                let tokens = ui.tokens();
                match status {
                    ConversationStatus::Notification { needs_input } => {
                        let hover_text = if *needs_input {
                            "Needs your input"
                        } else {
                            "New messages"
                        };
                        status_dot_ui(ui, tokens.info_text_color).on_hover_text(hover_text);
                    }
                    ConversationStatus::Errored { message } => {
                        status_dot_ui(ui, tokens.error_fg_color).on_hover_text(message);
                    }
                    ConversationStatus::Working => {
                        ui.inline_loading_indicator("The agent is working");
                    }
                    ConversationStatus::NothingNew => {}
                }
            },
        );
}

pub fn status_dot_ui(ui: &mut egui::Ui, color: egui::Color32) -> egui::Response {
    let radius = ui.tokens().status_dot_radius();
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(4.0 * radius), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), radius, color);
    response
}
