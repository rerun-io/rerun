//! The chat view: title bar with mode picker, the transcript, and the composer at the bottom
//! (login and permission prompts, plan, prompt input, agent log).

use egui::{Key, KeyboardShortcut, Modifiers, RichText};
use re_agent::acp::LineDirection;
use re_agent::acp::schema::v1::{AuthMethod, PermissionOptionKind, SessionMode};
use re_ui::alert::Alert;
use re_ui::{ReButton, UiExt as _, icons};

use super::tool_call_ui::{code_ui, tool_input_ui};
use super::transcript_ui::{plan_ui, transcript_ui};
use re_agent::{AgentSession, AuthPrompt, Phase};

/// What the user is typing, plus whether the text field should grab focus this frame.
#[derive(Default)]
pub struct ChatInput {
    /// The prompt being composed.
    pub text: String,
    focus: bool,

    /// Used to request focus when the agent becomes ready.
    was_ready: bool,

    /// Whether the agent log at the bottom is expanded.
    log_open: bool,
}

/// The whole chat view for one session: transcript on top, composer at the bottom.
/// `login_hint` is the terminal command that logs in to the agent, shown when it asks for auth.
pub fn chat_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    input: &mut ChatInput,
    show_thoughts: bool,
    login_hint: Option<&str>,
) {
    if session.is_ready() && !input.was_ready {
        input.focus = true;
    }
    input.was_ready = session.is_ready();

    egui::Panel::bottom(ui.id().with("agent_composer"))
        .frame(egui::Frame::new().inner_margin(8))
        .show(ui, |ui| {
            composer_ui(ui, session, input, login_hint);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    transcript_ui(ui, session.transcript(), show_thoughts);

                    if let Phase::Connecting { status } = session.phase() {
                        ui.add_space(8.0);
                        ui.loading_indicator(status);
                    }
                });
        });
}

fn mode_picker_ui(ui: &mut egui::Ui, session: &mut AgentSession) {
    let Some(modes) = session.modes() else {
        return;
    };
    let Some(current_id) = session.current_mode_id() else {
        return;
    };
    if modes.available_modes.is_empty() {
        return;
    }

    let current_name = modes
        .available_modes
        .iter()
        .find(|mode| &mode.id == current_id)
        .map_or_else(|| current_id.0.to_string(), |mode| mode.name.clone());

    let mut selected = None;
    ui.drop_down_menu("agent_mode", current_name, |ui| {
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
        session.set_mode(mode_id);
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
    } else {
        None
    }
}

fn composer_ui(
    ui: &mut egui::Ui,
    session: &mut AgentSession,
    input: &mut ChatInput,
    login_hint: Option<&str>,
) {
    ui.spacing_mut().item_spacing.y = 8.0;

    if let Some(auth) = session.auth()
        && let Some(method_id) = auth_ui(ui, auth, login_hint)
    {
        session.authenticate(method_id);
    }

    permissions_ui(ui, session, input);

    if let Some(plan) = &session.transcript().plan {
        plan_ui(ui, plan);
    }

    queue_ui(ui, session, input);
    input_ui(ui, session, input);

    footer_ui(ui, session, input);
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
                            ui.label(prompt.lines().next().unwrap_or_default());
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
fn move_into_input(input: &mut ChatInput, prompt: String) {
    if input.text.trim().is_empty() {
        input.text = prompt;
    } else {
        input.text = format!("{prompt}\n{}", input.text);
    }
    input.focus = true;
}

/// Stops the agent. Queued prompts go back into the input instead of starting new turns.
fn stop_agent(session: &mut AgentSession, input: &mut ChatInput) {
    for prompt in session.take_queued().into_iter().rev() {
        move_into_input(input, prompt);
    }
    session.cancel();
}

/// Mode picker, connection status or token usage, and the agent log toggle.
fn footer_ui(ui: &mut egui::Ui, session: &mut AgentSession, input: &mut ChatInput) {
    ui.horizontal(|ui| {
        mode_picker_ui(ui, session);

        match session.phase() {
            Phase::Connecting { status } => {
                ui.weak(status);
            }
            Phase::Ready => {
                if let Some(usage) = &session.transcript().usage {
                    let text = format!(
                        "{} / {} tokens",
                        format_tokens(usage.used),
                        format_tokens(usage.size)
                    );
                    ui.weak(text).on_hover_text("Context window usage");
                }
            }
            Phase::Disconnected { reason } => {
                ui.error_label(reason.clone());
            }
            Phase::Idle => {}
        }

        let log_lines = session.log().len();
        if log_lines != 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let text = RichText::new(format!("Agent log ({log_lines} lines)")).small();
                if ui
                    .selectable_label(input.log_open, text)
                    .on_hover_text("Show what the agent wrote to stderr")
                    .clicked()
                {
                    input.log_open = !input.log_open;
                }
            });
        }
    });

    if input.log_open {
        log_ui(ui, session);
    }
}

/// Returns the auth method the user picked, if any.
fn auth_ui(
    ui: &mut egui::Ui,
    auth: &AuthPrompt,
    login_hint: Option<&str>,
) -> Option<re_agent::acp::schema::v1::AuthMethodId> {
    let mut picked = None;
    Alert::info().show_with_title(ui, "Login required", |ui| {
            ui.label(&auth.message);
            if let Some(login_hint) = login_hint {
                ui.weak("The reliable way in is the agent's own CLI. Log in from a terminal, then restart the session:");
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
                            ui.weak("Run this in a terminal, then retry:");
                            code_ui(ui, &method.args.join(" "), None);
                        }
                        if ui
                            .add(ReButton::new(format!("Retry: {}", method.name)).primary())
                            .clicked()
                        {
                            picked = Some(method.id.clone());
                        }
                    }
                    AuthMethod::Agent(method) => {
                        if ui.add(ReButton::new(&method.name).primary()).clicked() {
                            picked = Some(method.id.clone());
                        }
                        if let Some(description) = &method.description {
                            ui.weak(description);
                        }
                    }
                    _ => {}
                }
            }
        });
    picked
}

fn permissions_ui(ui: &mut egui::Ui, session: &mut AgentSession, input: &mut ChatInput) {
    if session.pending_permissions().is_empty() {
        return;
    }

    // Keyboard shortcuts act on the oldest request only.
    let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    let escape = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));

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
                    .max_height(320.0)
                    .id_salt(&request.tool_call.tool_call_id.0)
                    .show(ui, |ui| tool_input_ui(ui, raw_input));
            }

            ui.horizontal_wrapped(|ui| {
                for option in &request.options {
                    let button = match option.kind {
                        PermissionOptionKind::AllowOnce => ReButton::new(&option.name).primary(),
                        _ => ReButton::new(&option.name).outlined(),
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

fn input_ui(ui: &mut egui::Ui, session: &mut AgentSession, input: &mut ChatInput) {
    slash_commands_ui(ui, session, input);

    let hint = if session.turn_in_progress() {
        "The agent is working… (⏎ to queue a message, Esc to stop)"
    } else if session.is_ready() {
        "Ask the agent… (⏎ to send, ⇧⏎ for a new line)"
    } else {
        "Waiting for the agent…"
    };

    let tokens = ui.tokens();
    let response = egui::Frame::new()
        .fill(tokens.text_edit_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            tokens.widget_noninteractive_bg_stroke,
        ))
        .corner_radius(6)
        .inner_margin(6)
        .show(ui, |ui| {
            let response = ui.add(
                egui::TextEdit::multiline(&mut input.text)
                    .id_salt("chat_input")
                    .hint_text(hint)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .frame(egui::Frame::new())
                    .return_key(Some(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter))),
            );

            if session.turn_in_progress() {
                ui.horizontal(|ui| {
                    ui.inline_loading_indicator("Working…");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(ReButton::new("Stop").outlined().small())
                            .on_hover_text("Cancel the current turn (Esc)")
                            .clicked()
                        {
                            stop_agent(session, input);
                        }
                    });
                });
            }

            response
        })
        .inner;

    if std::mem::take(&mut input.focus) {
        response.request_focus();
    }

    if response.has_focus() {
        let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        if enter && session.send_prompt(input.text.trim()) {
            input.text.clear();
            response.request_focus();
        }
    }

    // egui takes focus away from the text edit on Escape before any widget runs,
    // so by now the input has already lost it. Take it back: Escape means "stop the agent" here.
    // TODO(emilk): use `TextEdit::event_filter` to keep focus on Escape instead, once we update to
    // the egui release containing <https://github.com/emilk/egui/pull/8529>.
    if response.has_focus() || response.lost_focus() {
        let escape = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        if escape {
            if session.turn_in_progress() {
                stop_agent(session, input);
            }
            response.request_focus();
        }
    }
}

/// Lists matching slash commands while the input starts with `/`.
///
/// TODO(emilk): replace with `egui::CompletionPopup` (arrow keys, Tab, Escape) when we update to
/// the egui release containing <https://github.com/emilk/egui/pull/8529>.
fn slash_commands_ui(ui: &mut egui::Ui, session: &AgentSession, input: &mut ChatInput) {
    let Some(query) = input.text.strip_prefix('/') else {
        return;
    };
    if query.contains(char::is_whitespace) {
        return;
    }

    let matching: Vec<_> = session
        .transcript()
        .available_commands
        .iter()
        .filter(|command| command.name.starts_with(query))
        .take(8)
        .collect();
    if matching.is_empty() {
        return;
    }

    let mut completion = None;
    let tokens = ui.tokens();
    egui::Frame::new()
        .fill(tokens.floating_color)
        .corner_radius(6)
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for command in matching {
                let response = ui.horizontal(|ui| {
                    let response = ui.selectable_label(
                        false,
                        RichText::new(format!("/{}", command.name)).monospace(),
                    );
                    ui.weak(&command.description);
                    response
                });
                if response.inner.clicked() {
                    completion = Some(format!("/{} ", command.name));
                }
            }
        });

    if let Some(completion) = completion {
        input.text = completion;
        input.focus = true;
    }
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
                let id = ui.id().with(index);
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

fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    }
}
