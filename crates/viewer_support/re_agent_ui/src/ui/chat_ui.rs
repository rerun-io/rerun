//! The chat view: the transcript, and the composer at the bottom
//! (login and permission prompts, plan, prompt input with its footer, agent log).

use std::sync::Arc;

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{EventFilter, Key, KeyboardShortcut, Modifiers, RichText};
use re_agent::LineDirection;
use re_agent::acp::schema::v1::{
    AuthMethod, PermissionOptionKind, SessionConfigSelectOption, SessionConfigSelectOptions,
    SessionConfigValueId, SessionMode, SessionModeId,
};
use re_ui::alert::Alert;
use re_ui::egui_ext::{CompletionPopup, CompletionQuery, Suggestion};
use re_ui::{ButtonExt as _, UiExt as _, icons};

use super::MAX_REQUEST_CONTENT_HEIGHT;
use super::elicitation_ui::ElicitationUi;
use super::pasted_image;
use super::tool_call_ui::{code_ui, tool_input_ui};
use super::transcript_ui::{plan_ui, transcript_ui};
use re_agent::{AgentSession, AuthPrompt, Phase, Prompt, PromptImage};

/// Height a pasted image is previewed at in the composer, in points.
const PREVIEW_HEIGHT: f32 = 48.0;

const EMPTY_STATE_WIDTH: f32 = 240.0;

/// What the user is typing, plus whether the text field should grab focus this frame.
#[derive(Default)]
pub struct ChatInput {
    pub restart_after_login_requested: bool,

    /// An example prompt the user clicked, to be sent the way a host prompt is.
    pub example_prompt_clicked: Option<String>,

    /// The prompt being composed: what is typed, and the images pasted in.
    pub prompt: Prompt,
    focus: bool,

    /// Used to request focus when the agent becomes ready.
    was_ready: bool,

    /// Whether the agent log at the bottom is expanded.
    log_open: bool,

    /// Whether the slash command popup was open last frame, so the arrow keys go to it
    /// instead of the prompt history.
    completion_open: bool,

    history: PromptHistory,

    /// Thumbnails of [`Prompt::images`] in [`Self::prompt`].
    previews: ImagePreviews,
    elicitation_ui: ElicitationUi,
}

/// Textures for the images attached to the prompt being composed.
///
/// Keyed by the image's shared bytes. Each entry keeps those bytes alive, so their address cannot
/// be reused by another image while the entry exists.
#[derive(Default)]
struct ImagePreviews {
    /// `None` for an image that failed to decode, so it is not decoded again every frame.
    textures: Vec<(Arc<[u8]>, Option<egui::TextureHandle>)>,
}

impl ImagePreviews {
    fn insert(&mut self, image: &PromptImage, texture: egui::TextureHandle) {
        self.textures.push((image.bytes.clone(), Some(texture)));
    }

    /// The texture for `image`, decoding and uploading it the first time it is asked for.
    ///
    /// `None` if it cannot be decoded.
    fn get(&mut self, ctx: &egui::Context, image: &PromptImage) -> Option<egui::TextureId> {
        if let Some((_, texture)) = self
            .textures
            .iter()
            .find(|(bytes, _)| Arc::ptr_eq(bytes, &image.bytes))
        {
            return texture.as_ref().map(egui::TextureHandle::id);
        }
        let texture = pasted_image::decode(image)
            .map(|pixels| ctx.load_texture("pasted_image", pixels, egui::TextureOptions::LINEAR));
        let id = texture.as_ref().map(egui::TextureHandle::id);
        self.textures.push((image.bytes.clone(), texture));
        id
    }

    /// Frees the textures of images that are no longer attached.
    fn retain(&mut self, images: &[PromptImage]) {
        self.textures
            .retain(|(bytes, _)| images.iter().any(|image| Arc::ptr_eq(bytes, &image.bytes)));
    }
}

impl ChatInput {
    /// Give the prompt input keyboard focus the next time it is shown.
    pub fn request_focus(&mut self) {
        self.focus = true;
    }

    /// Starts over for a new session, except for the prompt being composed and a pending focus
    /// request.
    pub fn reset_keeping_prompt(&mut self) {
        *self = Self {
            prompt: std::mem::take(&mut self.prompt),
            previews: std::mem::take(&mut self.previews),
            focus: self.focus,
            ..Default::default()
        };
    }
}

/// Prompts sent so far, browsed with the arrow keys the way a shell browses its history.
#[derive(Default)]
struct PromptHistory {
    /// Oldest first. Consecutive duplicates are stored once.
    entries: Vec<String>,

    /// Index into [`Self::entries`] while browsing; `None` while composing a new prompt.
    position: Option<usize>,

    /// The unsent text saved when browsing started, restored when browsing past the newest entry.
    draft: String,
}

impl PromptHistory {
    fn push(&mut self, prompt: &str) {
        if self.entries.last().is_none_or(|last| last != prompt) {
            self.entries.push(prompt.to_owned());
        }
        self.position = None;
        self.draft.clear();
    }

    /// The next older entry, saving `current` as the draft when browsing starts.
    fn back(&mut self, current: &str) -> Option<String> {
        let position = match self.position {
            None => {
                let position = self.entries.len().checked_sub(1)?;
                self.draft = current.to_owned();
                position
            }
            Some(0) => return None,
            Some(position) => position - 1,
        };
        self.position = Some(position);
        Some(self.entries[position].clone())
    }

    /// The next newer entry, or the draft when moving past the newest one.
    fn forward(&mut self) -> Option<String> {
        let position = self.position? + 1;
        if position < self.entries.len() {
            self.position = Some(position);
            Some(self.entries[position].clone())
        } else {
            self.position = None;
            Some(std::mem::take(&mut self.draft))
        }
    }
}

/// The whole chat view for one session: transcript on top, composer at the bottom.
/// `login_hint` is the terminal command that logs in to the agent, shown when it asks for auth.
/// `home_ui`, if given, is shown above the composer instead of the transcript.
pub fn chat_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    input: &mut ChatInput,
    preferred_mode: &mut Option<SessionModeId>,
    preferred_model: &mut Option<SessionConfigValueId>,
    show_thoughts: bool,
    login_hint: Option<&str>,
    example_prompts: &[String],
    home_ui: Option<&mut dyn FnMut(&mut egui::Ui)>,
) {
    if session.is_ready() && !input.was_ready {
        input.focus = true;
    }
    input.was_ready = session.is_ready();

    egui::Panel::bottom("agent_composer")
        .frame(egui::Frame::new().inner_margin(8))
        .show(ui, |ui| {
            composer_ui(
                ui,
                session,
                input,
                preferred_mode,
                preferred_model,
                login_hint,
            );
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 12,
            bottom: 4,
        }))
        .show(ui, |ui| {
            if let Some(home_ui) = home_ui {
                home_ui(ui);
                return;
            }
            if session.transcript().items.is_empty()
                && !matches!(session.phase(), Phase::Connecting { .. })
            {
                empty_state_ui(ui, input, example_prompts);
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    transcript_ui(ui, session, show_thoughts);

                    if let Phase::Connecting { status } = session.phase()
                        && !session.is_waiting_on_user()
                    {
                        ui.add_space(8.0);
                        ui.loading_indicator(status);
                    }
                });
        });
}

fn empty_state_ui(ui: &mut egui::Ui, input: &mut ChatInput, example_prompts: &[String]) {
    let content_height = ui
        .new_child(egui::UiBuilder::new().sizing_pass().invisible())
        .vertical_centered(|ui| empty_state_content_ui(ui, input, example_prompts))
        .response
        .rect
        .height();
    ui.add_space(((ui.available_height() - content_height) / 2.0).max(0.0));
    ui.vertical_centered(|ui| empty_state_content_ui(ui, input, example_prompts));
}

fn empty_state_content_ui(ui: &mut egui::Ui, input: &mut ChatInput, example_prompts: &[String]) {
    ui.set_max_width(EMPTY_STATE_WIDTH);
    ui.add(
        egui::Label::new(
            RichText::new(
                "The Rerun agent helps you solve complex physical AI problems right within the viewer.",
            )
            .weak(),
        )
        .halign(egui::Align::Center)
        .wrap(),
    );

    let tokens = ui.tokens();
    egui::WidgetAtom::new(RichText::new("Experimental").small())
        .frame(
            egui::Frame::new()
                .fill(tokens.example_tag_bg_fill)
                .stroke(tokens.example_tag_stroke)
                .corner_radius(tokens.normal_corner_radius())
                .inner_margin(egui::Margin::symmetric(8, 2)),
        )
        .show(ui);

    ui.add_space(ui.spacing().item_spacing.y);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    for prompt in example_prompts {
        if ui
            .add(egui::Button::new(prompt).outlined().size_small())
            .clicked()
        {
            input.example_prompt_clicked = Some(prompt.clone());
        }
    }
}

fn mode_picker_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    preferred_mode: &mut Option<SessionModeId>,
) {
    let Some(modes) = session.modes() else {
        return;
    };
    let Some(current_id) = session.current_mode_id() else {
        return;
    };
    if modes.available_modes.is_empty() {
        return;
    }

    let current_mode = modes
        .available_modes
        .iter()
        .find(|mode| &mode.id == current_id);
    let mut current_text = RichText::new(
        current_mode.map_or_else(|| current_id.0.to_string(), |mode| mode.name.clone()),
    );
    if let Some(color) = current_mode.and_then(|mode| mode_warning_color(ui, mode)) {
        current_text = current_text.color(color);
    }

    let mut selected = None;
    ui.drop_down_menu("agent_mode", current_text, |ui| {
        for mode in &modes.available_modes {
            let mut text = RichText::new(&mode.name);
            if let Some(color) = mode_warning_color(ui, mode) {
                text = text.color(color);
            }
            let response = ui
                .list_item()
                .selected(&mode.id == current_id)
                .show_flat(ui, re_ui::list_item::LabelContent::new(text));
            let response = match &mode.description {
                Some(description) => response.on_hover_text(description),
                None => response,
            };
            if response.clicked() {
                selected = Some(mode.id.clone());
            }
        }
    })
    .on_hover_text("Session mode: how much the agent may do without asking");

    if let Some(mode_id) = selected {
        *preferred_mode = Some(mode_id.clone());
        session.set_mode(mode_id);
    }
}

/// The model the agent is using, as a drop-down to switch to another one it offers.
fn model_picker_ui(
    ui: &mut egui::Ui,
    session: &AgentSession,
    preferred_model: &mut Option<SessionConfigValueId>,
) {
    let transcript = session.transcript();
    let (Some((config_id, select)), Some(current_name)) =
        (transcript.model_selector(), transcript.current_model())
    else {
        return;
    };

    let model_item = |ui: &mut egui::Ui, model: &SessionConfigSelectOption| {
        let response = ui
            .list_item()
            .selected(model.value == select.current_value)
            .show_flat(ui, re_ui::list_item::LabelContent::new(&model.name));
        let response = match &model.description {
            Some(description) => response.on_hover_text(description),
            None => response,
        };
        response.clicked()
    };

    let mut selected = None;
    ui.drop_down_menu("agent_model", current_name.to_owned(), |ui| {
        match &select.options {
            SessionConfigSelectOptions::Ungrouped(models) => {
                for model in models {
                    if model_item(ui, model) {
                        selected = Some(model.value.clone());
                    }
                }
            }
            SessionConfigSelectOptions::Grouped(groups) => {
                for group in groups {
                    ui.weak(&group.name);
                    for model in &group.options {
                        if model_item(ui, model) {
                            selected = Some(model.value.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    })
    .on_hover_text("The model the agent is using");

    if let Some(value) = selected {
        *preferred_model = Some(value.clone());
        if value != select.current_value {
            session.set_config_option(config_id.clone(), value);
        }
    }
}

/// Modes that let the agent act without asking are shown in warning colors.
///
/// ACP has no flag for this, so we go by the mode's id and name.
fn mode_warning_color(ui: &egui::Ui, mode: &SessionMode) -> Option<egui::Color32> {
    let haystack = format!("{} {}", mode.id.0, mode.name).to_lowercase();
    let tokens = ui.tokens();
    if ["bypass", "yolo", "danger", "full", "never"]
        .iter()
        .any(|needle| haystack.contains(needle))
    {
        Some(tokens.error_fg_color)
    } else if haystack.contains("auto") || haystack.contains("approve") {
        Some(tokens.warn_fg_color)
    } else if haystack.contains("plan") {
        Some(tokens.info_text_color)
    } else {
        None
    }
}

fn composer_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    input: &mut ChatInput,
    preferred_mode: &mut Option<SessionModeId>,
    preferred_model: &mut Option<SessionConfigValueId>,
    login_hint: Option<&str>,
) {
    ui.spacing_mut().item_spacing.y = 8.0;

    if let Some(auth) = session.auth()
        && let Some(method_id) = auth_ui(
            ui,
            auth,
            login_hint,
            &mut input.restart_after_login_requested,
        )
    {
        session.authenticate(method_id);
    }

    permissions_ui(ui, session, input);
    if input.elicitation_ui.ui(ui, session) {
        input.focus = true;
    }

    if let Some(plan) = &session.transcript().plan {
        plan_ui(ui, plan);
    }

    queue_ui(ui, session, input);
    input_ui(ui, session, input, preferred_mode, preferred_model);

    if input.log_open {
        log_ui(ui, session);
    }
}

/// Prompts waiting for the running turn to finish. Each can be taken back into the input.
fn queue_ui(ui: &mut egui::Ui, session: &mut AgentSession, input: &mut ChatInput) {
    if session.queued_prompts().is_empty() {
        return;
    }

    let tokens = ui.tokens();
    let mut take_back = None;
    egui::Frame::new()
        .fill(tokens.faint_bg_color)
        .corner_radius(6)
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            for (index, prompt) in session.queued_prompts().iter().enumerate() {
                egui::containers::Sides::new()
                    .shrink_left()
                    .truncate()
                    .show(
                        ui,
                        |ui| {
                            ui.weak(if index == 0 { "Next:" } else { "Then:" });
                            ui.label(queued_summary(prompt));
                        },
                        |ui| {
                            if ui
                                .small_icon_button(&icons::EDIT, "Move back into the input")
                                .on_hover_text("Move back into the input")
                                .clicked()
                            {
                                take_back = Some(index);
                            }
                        },
                    );
            }
        });

    if let Some(index) = take_back
        && let Some(prompt) = session.remove_queued(index)
    {
        move_into_input(input, prompt);
    }
}

/// Puts `prompt` in front of whatever is being typed, and focuses the input.
///
/// Its images go back to the front of the pending ones, so sending again attaches them in the
/// order they were pasted in.
fn move_into_input(input: &mut ChatInput, prompt: Prompt) {
    let Prompt { text, images } = prompt;
    if input.prompt.text.trim().is_empty() {
        input.prompt.text = text;
    } else {
        input.prompt.text = format!("{text}\n{}", input.prompt.text);
    }
    input.prompt.images.splice(..0, images);
    input.focus = true;
}

/// One line describing a queued prompt, for the list of what is waiting.
fn queued_summary(prompt: &Prompt) -> String {
    let Prompt { text, images } = prompt;
    let first_line = text.lines().next().unwrap_or_default();
    match images.len() {
        0 => first_line.to_owned(),
        1 => format!("{first_line} [1 image]"),
        count => format!("{first_line} [{count} images]"),
    }
}

/// Stops the agent. Queued prompts go back into the input instead of starting new turns.
fn stop_agent(session: &mut AgentSession, input: &mut ChatInput) {
    for prompt in session.take_queued().into_iter().rev() {
        move_into_input(input, prompt);
    }
    session.cancel();
}

/// The row at the bottom of the input: mode picker, model, connection status, and the agent
/// log toggle on the left; context usage and the send or stop button on the right.
fn footer_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    input: &mut ChatInput,
    preferred_mode: &mut Option<SessionModeId>,
    preferred_model: &mut Option<SessionConfigValueId>,
) {
    let usage = session
        .transcript()
        .usage
        .as_ref()
        .filter(|_| *session.phase() == Phase::Ready)
        .map(|usage| (usage.used, usage.size));
    let turn_in_progress = session.turn_in_progress();
    let can_send = session.is_ready()
        && (!input.prompt.text.trim().is_empty() || !input.prompt.images.is_empty());

    let ((), clicked) = egui::containers::Sides::new()
        .shrink_left()
        .truncate()
        .show(
            ui,
            |ui| {
                // In a narrow panel, truncated labels still paint their "…" past the edge.
                // Use a stricter `clip_rect` to avoid overlap with the right side UI.
                let mut clip_rect = ui.clip_rect();
                clip_rect.max.x = clip_rect.max.x.min(ui.max_rect().max.x);
                ui.set_clip_rect(clip_rect);
                mode_picker_ui(ui, session, preferred_mode);

                model_picker_ui(ui, session, preferred_model);

                match session.phase() {
                    Phase::Connecting { status } => {
                        ui.weak(status);
                    }
                    Phase::Disconnected { reason } => {
                        ui.error_label(reason.clone());
                    }
                    Phase::Idle | Phase::Ready => {}
                }

                let log_lines = session.log().len();
                if log_lines != 0 {
                    let text = RichText::new(format!("Agent log ({log_lines} lines)")).small();
                    if ui
                        .selectable_label(input.log_open, text)
                        .on_hover_text("Show what the agent wrote to stderr")
                        .clicked()
                    {
                        input.log_open = !input.log_open;
                    }
                }
            },
            |ui| {
                let clicked = if turn_in_progress {
                    ui.add(
                        egui::Button::icon(icons::STOP, "Stop")
                            .primary()
                            .size_small(),
                    )
                    .on_hover_text("Cancel the current turn (Esc)")
                    .clicked()
                } else {
                    ui.add_enabled(
                        can_send,
                        egui::Button::icon(icons::ARROW_UP, "Send")
                            .primary()
                            .size_small(),
                    )
                    .on_hover_text("Send (⏎)")
                    .clicked()
                };
                if let Some((used_tokens, max_tokens)) = usage {
                    context_usage_ui(ui, used_tokens, max_tokens);
                }
                clicked
            },
        );

    if clicked {
        if turn_in_progress {
            stop_agent(session, input);
        } else {
            send_input(session, input);
            input.focus = true;
        }
    }
}

/// Returns the auth method the user picked, if any.
fn auth_ui(
    ui: &mut egui::Ui,
    auth: &AuthPrompt,
    login_hint: Option<&str>,
    restart_after_login_requested: &mut bool,
) -> Option<re_agent::acp::schema::v1::AuthMethodId> {
    let mut picked = None;
    Alert::info().show_with_title(ui, "Login required", |ui| {
            ui.label(&auth.message);
            let has_login_method = auth.methods.iter().any(|method| {
                matches!(method, AuthMethod::Terminal(_) | AuthMethod::Agent(_))
            });
            if !has_login_method && let Some(login_hint) = login_hint {
                ui.weak("Log in from a terminal, then restart the agent:");
                ui.push_id("login_hint", |ui| code_ui(ui, login_hint, None));
            }
            for method in &auth.methods {
                match method {
                    AuthMethod::Terminal(method) => {
                        ui.label(&method.name);
                        if let Some(description) = &method.description {
                            ui.weak(description);
                        }
                        if !method.args.is_empty() {
                            ui.weak("Run this in a terminal, then restart the agent:");
                            code_ui(ui, &method.args.join(" "), None);
                        }
                    }
                    AuthMethod::Agent(method) => {
                        if ui.add(egui::Button::new(&method.name).primary().size_small()).clicked() {
                            picked = Some(method.id.clone());
                        }
                        if let Some(description) = &method.description {
                            ui.weak(description);
                        }
                    }
                    _ => {}
                }
            }
            // NOTE: Retrying within the session would be preferable, but may not be supported by all agents.
            ui.weak("Restarting clears this conversation and resends your last prompt in a new session, without the previous conversation context.");
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Restart and resend").primary().size_small())
                    .on_hover_text("Restarts the agent to pick up your credentials and resends your last prompt, including attachments, in the new session.")
                    .clicked()
                {
                    *restart_after_login_requested = true;
                }
                });
            });
        });
    picked
}

fn permissions_ui(ui: &mut egui::Ui, session: &mut AgentSession, input: &mut ChatInput) {
    if session.pending_permissions().is_empty() {
        return;
    }

    // Keyboard shortcuts act on the oldest request only, while the chat input has focus.
    // While a question is open, Enter and Escape must not answer a tool approval instead.
    let input_id = ui.make_persistent_id("chat_input");
    let shortcuts_enabled =
        session.pending_elicitations().is_empty() && ui.memory(|memory| memory.has_focus(input_id));
    let enter = shortcuts_enabled && consume_plain_enter(ui);
    let escape = shortcuts_enabled && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));

    let mut answered = None;
    for (index, pending) in session.pending_permissions().iter().enumerate() {
        let is_first = index == 0;
        let request = &pending.request;
        let known_call = session
            .transcript()
            .tool_call(&request.tool_call.tool_call_id);

        Alert::warning().show_with_title(ui, "Permission requested", |ui| {
            let title = request
                .tool_call
                .fields
                .title
                .clone()
                .or_else(|| known_call.map(|call| call.title.clone()))
                .unwrap_or_else(|| "Tool call".to_owned());
            ui.label(RichText::new(title).monospace());

            let raw_input = request
                .tool_call
                .fields
                .raw_input
                .as_ref()
                .or_else(|| known_call?.raw_input.as_ref());
            if let Some(raw_input) = raw_input {
                egui::ScrollArea::vertical()
                    .max_height(MAX_REQUEST_CONTENT_HEIGHT)
                    .id_salt(&request.tool_call.tool_call_id.0)
                    .show(ui, |ui| tool_input_ui(ui, raw_input));
            }

            ui.horizontal_wrapped(|ui| {
                for option in &request.options {
                    let button = match option.kind {
                        PermissionOptionKind::AllowOnce => {
                            egui::Button::new(&option.name).primary().size_small()
                        }
                        _ => egui::Button::new(&option.name).outlined().size_small(),
                    };
                    let clicked = ui.add(button).clicked();
                    let shortcut = is_first
                        && match option.kind {
                            PermissionOptionKind::AllowOnce => enter,
                            PermissionOptionKind::RejectOnce => escape,
                            _ => false,
                        };
                    if (clicked || shortcut) && answered.is_none() {
                        answered = Some((index, option.option_id.clone()));
                    }
                }
            });
            if is_first {
                ui.weak("⏎ allow once · Esc reject once");
            }
        });
    }

    if let Some((index, option_id)) = answered {
        session.answer_permission(index, Some(option_id));
        input.focus = true;
    }
}

fn input_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    input: &mut ChatInput,
    preferred_mode: &mut Option<SessionModeId>,
    preferred_model: &mut Option<SessionConfigValueId>,
) {
    let hint = if session.turn_in_progress() {
        "The agent is working… (⏎ to queue a message, Esc to stop)"
    } else if session.is_ready() {
        "Ask the agent… (⏎ to send, ⇧⏎ for a new line)"
    } else {
        "Waiting for the agent…"
    };

    let input_id = ui.make_persistent_id("chat_input");
    history_navigation(ui, input, input_id);

    let tokens = ui.tokens();
    let response = egui::Frame::new()
        .fill(tokens.text_edit_bg_color)
        .stroke(tokens.control_stroke)
        .corner_radius(6)
        .inner_margin(6)
        .show(ui, |ui| {
            pending_images_ui(ui, input);

            let output = CompletionPopup::new(input_id)
                .event_filter(EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                })
                .show(
                    ui,
                    &mut input.prompt.text,
                    |text| {
                        egui::TextEdit::multiline(text)
                            .hint_text(hint)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .frame(egui::Frame::new())
                            .return_key(Some(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                    },
                    |query| slash_command_suggestions(session, query),
                );
            input.completion_open = output.is_open;
            let response = output.text_edit.response.response;

            footer_ui(ui, session, input, preferred_mode, preferred_model);

            response
        })
        .inner;

    if std::mem::take(&mut input.focus) {
        response.request_focus();
    }

    if response.has_focus() {
        take_pasted_images(ui, input);

        if consume_plain_enter(ui) && send_input(session, input) {
            // Keep focus after hitting enter to send, so the next message can be typed directly.
            response.request_focus();
        }

        // The event filter keeps focus on Escape: here it means "stop the agent", not "leave the input".
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
            && session.turn_in_progress()
        {
            stop_agent(session, input);
        }
    }
}

/// Sends what is in the input, or queues it while a turn is running.
///
/// Returns `true` if the prompt was taken, which also clears the input.
fn send_input(session: &mut AgentSession, input: &mut ChatInput) -> bool {
    let prompt = Prompt {
        text: input.prompt.text.trim().to_owned(),
        images: input.prompt.images.clone(),
    };
    let sent = session.send_prompt(prompt.clone());
    if sent {
        input.history.push(&prompt.text);
        input.prompt = Prompt::default();
    }
    sent
}

/// Takes the images pasted this frame into the composer.
///
/// The events are consumed, not just read: a `TextEdit` ignores them, but leaving them in would
/// hand the same paste to every other widget that looks.
fn take_pasted_images(ui: &egui::Ui, input: &mut ChatInput) {
    let pasted = ui.input_mut(|state| {
        let mut pasted = Vec::new();
        state.events.retain(|event| {
            if let egui::Event::PasteImage(image) = event {
                pasted.push(Arc::clone(image));
                false
            } else {
                true
            }
        });
        pasted
    });

    for image in pasted {
        let Some((image, preview)) = pasted_image::encode(&image) else {
            re_log::warn!("Could not encode the pasted image");
            continue;
        };
        let preview = ui
            .ctx()
            .load_texture("pasted_image", preview, egui::TextureOptions::LINEAR);
        input.previews.insert(&image, preview);
        input.prompt.images.push(image);
    }
}

/// The images attached to the prompt being composed, each with a button to drop it again.
fn pending_images_ui(ui: &mut egui::Ui, input: &mut ChatInput) {
    let ChatInput {
        prompt, previews, ..
    } = input;
    previews.retain(&prompt.images);
    if prompt.images.is_empty() {
        return;
    }

    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, image) in prompt.images.iter().enumerate() {
            let [width, height] = image.resolution;
            let aspect = width as f32 / height as f32;
            let size = egui::vec2(PREVIEW_HEIGHT * aspect, PREVIEW_HEIGHT);
            let hover_text = format!("Pasted image, {width}×{height} pixels");
            if let Some(texture) = previews.get(ui.ctx(), image) {
                ui.image((texture, size)).on_hover_text(hover_text);
            } else {
                ui.weak("(image)").on_hover_text(hover_text);
            }
            if ui
                .small_icon_button(&icons::CLOSE_SMALL, "Remove this image")
                .on_hover_text("Do not send this image")
                .clicked()
            {
                remove = Some(index);
            }
        }
    });

    if let Some(index) = remove {
        prompt.images.remove(index);
    }
    ui.add_space(4.0);
}

/// Consume a press of `Enter` with no modifier held at all.
///
/// [`egui::InputState::consume_key`] matches modifiers logically, which ignores an extra shift or
/// alt, so it also fires on the modified `Enter` that adds a newline.
fn consume_plain_enter(ui: &egui::Ui) -> bool {
    ui.input_mut(|input| {
        let mut pressed = false;
        input.events.retain(|event| {
            let plain_enter = matches!(
                event,
                egui::Event::Key {
                    key: Key::Enter,
                    modifiers,
                    pressed: true,
                    ..
                } if modifiers.is_none()
            );
            pressed |= plain_enter;
            !plain_enter
        });
        pressed
    })
}

/// Up on the first line recalls the previous prompt, down on the last line the next one,
/// like a shell. Runs before the text edit so the key does not also move the cursor.
fn history_navigation(ui: &egui::Ui, input: &mut ChatInput, input_id: egui::Id) {
    if input.completion_open || !ui.memory(|memory| memory.has_focus(input_id)) {
        return;
    }
    let ChatInput {
        prompt: Prompt { text, .. },
        history,
        ..
    } = input;

    let mut state = TextEditState::load(ui.ctx(), input_id).unwrap_or_default();
    let cursor = state
        .cursor
        .char_range()
        .map_or_else(|| text.chars().count(), |range| range.primary.index.into());
    let byte_offset = text
        .char_indices()
        .nth(cursor)
        .map_or(text.len(), |(offset, _)| offset);
    let on_first_line = !text[..byte_offset].contains('\n');
    let on_last_line = !text[byte_offset..].contains('\n');

    let up =
        on_first_line && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp));
    let down = !up
        && on_last_line
        && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown));

    let recalled = if up {
        history.back(text)
    } else if down {
        history.forward()
    } else {
        None
    };
    if let Some(recalled) = recalled {
        *text = recalled;
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(text.chars().count()))));
        state.store(ui.ctx(), input_id);
    }
}

/// The slash commands the agent offers, while the cursor is on a leading `/word`.
fn slash_command_suggestions(
    session: &AgentSession,
    query: &CompletionQuery<'_>,
) -> Vec<Suggestion> {
    let Some(prefix) = query.word.strip_prefix('/') else {
        return vec![];
    };
    if !query.is_at_start() {
        return vec![];
    }
    session
        .transcript()
        .available_commands
        .iter()
        .filter(|command| command.name.starts_with(prefix))
        .map(|command| {
            Suggestion::new(format!("/{} ", command.name))
                .content(RichText::new(format!("/{}", command.name)).monospace())
                .description(&command.description)
        })
        .collect()
}

fn log_ui(ui: &mut egui::Ui, session: &AgentSession) {
    let tokens = ui.tokens();
    egui::ScrollArea::vertical()
        .max_height(200.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (index, line) in session.log().iter().enumerate() {
                let (prefix, color) = match line.direction {
                    LineDirection::Stdin => ("→ ", tokens.text_subdued),
                    LineDirection::Stdout => ("← ", tokens.text_subdued),
                    LineDirection::Stderr => ("", tokens.warn_fg_color),
                };
                let id = ui.make_persistent_id(index);
                let is_expanded = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
                let wrap_mode = if is_expanded {
                    egui::TextWrapMode::Wrap
                } else {
                    egui::TextWrapMode::Truncate
                };
                let response = ui.add(
                    egui::Label::new(
                        RichText::new(format!("{prefix}{}", line.line))
                            .monospace()
                            .small()
                            .color(color),
                    )
                    .wrap_mode(wrap_mode)
                    .show_tooltip_when_elided(false)
                    .sense(egui::Sense::click())
                    .selectable(is_expanded),
                );
                if response.clicked() {
                    ui.data_mut(|data| data.insert_temp(id, !is_expanded));
                }
            }
        });
}

/// How full the agent's context window is, as a ring with the numbers in its tooltip.
fn context_usage_ui(ui: &mut egui::Ui, used_tokens: u64, max_tokens: u64) {
    let fraction = if max_tokens == 0 {
        0.0
    } else {
        used_tokens as f32 / max_tokens as f32
    };
    let description = format!(
        "Context window: {} / {} tokens ({:.0}%)",
        re_format::format_uint(used_tokens),
        re_format::format_uint(max_tokens),
        100.0 * fraction
    );
    ui.add(re_ui::ProgressRing::new(fraction).label(description.clone()))
        .on_hover_text(description);
}

#[cfg(test)]
mod tests {
    use super::PromptHistory;

    #[test]
    fn history_browsing() {
        let mut history = PromptHistory::default();
        assert_eq!(history.back("draft"), None);
        assert_eq!(history.forward(), None);

        history.push("one");
        history.push("two");
        history.push("two");
        assert_eq!(history.entries, vec!["one", "two"]);

        assert_eq!(history.back("draft").as_deref(), Some("two"));
        assert_eq!(history.back("ignored").as_deref(), Some("one"));
        assert_eq!(history.back("ignored"), None);
        assert_eq!(history.forward().as_deref(), Some("two"));
        assert_eq!(history.forward().as_deref(), Some("draft"));
        assert_eq!(history.forward(), None);

        history.back("");
        history.push("three");
        assert_eq!(history.position, None);
        assert_eq!(history.back("").as_deref(), Some("three"));
    }
}
