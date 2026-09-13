//! The setup screen: pick an agent, a working directory, MCP servers, and options.

use egui::RichText;
use re_ui::alert::Alert;
use re_ui::egui_ext::card_layout::{CardLayout, CardLayoutItem};
use re_ui::text_edit::ReTextEdit;
use re_ui::{ReButton, UiExt as _, icons};

use super::Screen;
use re_agent::AgentEntry;
use re_agent::{AgentSettings, McpServerConfig};

const CARD_MIN_WIDTH: f32 = 180.0;
const CARD_INNER_MARGIN: i8 = 10;

/// What prompt sharing actually does, behind the checkbox and the "What is shared?" hint.
const ANALYTICS_DETAILS: &str = "Prompts and the agent's answers go to Rerun only after a second \
     session of your own agent has removed names, identifiers, paths, and secrets, which spends \
     your own quota.\n\
     Analytics are processed on Rerun's behalf by PostHog, on servers in the EU. They are not \
     sold, and not shared with anyone else.\n\
     Content-free usage statistics are sent whether or not this is on. `rerun analytics disable` turns off all analytics.";

/// Returns [`Screen::Chat`] when the user clicked "OK", so the caller should start the agent.
#[must_use]
pub fn setup_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
) -> Screen {
    let mut next_screen = Screen::Setup;

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            egui::Frame::new().inner_margin(12).show(ui, |ui| {
                next_screen = setup_ui_inner(ui, settings, agents);
            });
        });

    next_screen
}

fn setup_ui_inner(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
) -> Screen {
    let mut next_screen = Screen::Setup;

    ui.spacing_mut().item_spacing.y = 8.0;

    warning_ui(ui);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Choose your agent").strong().size(15.0));
        if ui
            .small_icon_button(&icons::RESET, "Re-check which agents are installed")
            .on_hover_text("Re-check which agents are installed")
            .clicked()
        {
            for agent in agents.iter_mut() {
                agent.refresh();
            }
        }
    });

    ui.horizontal_top(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.small_icon(&icons::WARNING, Some(ui.tokens().alert_warning.icon));
        ui.label(
            "Runs on this machine using your own login and subscription. \
             The agent can reach your MCP servers and skills, and read and change files in its working directory.",
        );
    });

    agent_cards_ui(ui, settings, agents);

    prompt_analytics_ui(ui, &mut settings.share_redacted_prompts);

    ui.add_space(4.0);
    re_ui::list_item::list_item_scope(ui, "advanced", |ui| {
        ui.section_collapsing_header("Advanced")
            .default_open(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;

                ui.label(RichText::new("Working directory").strong());
                let current_dir = std::env::current_dir()
                    .map_or_else(|_| String::new(), |dir| dir.display().to_string());
                ui.add(ReTextEdit::singleline(&mut settings.cwd).hint_text(current_dir));
                ui.weak("The agent reads and edits files here. Point it at the project you want help with.");

                ui.add_space(8.0);
                ui.label(RichText::new("Environment variables").strong());
                ui.add(
                    ReTextEdit::multiline(&mut settings.env_vars)
                        .hint_text("CLAUDE_CONFIG_DIR=~/.claude-work"),
                );
                ui.weak("One NAME=value per line, set for the agent process.");

                ui.add_space(8.0);
                mcp_servers_ui(ui, &mut settings.mcp_servers);

                ui.add_space(8.0);
                ui.label(RichText::new("Options").strong());
                ui.re_checkbox(&mut settings.show_thoughts, "Show the agent's thinking");
                ui.re_checkbox(
                    &mut settings.log_protocol,
                    "Log every protocol message (for debugging)",
                );
            });
    });

    ui.add_space(8.0);

    let ready = selected_agent_ready(settings, agents);
    let label = format!("Start {}", selected_agent_name(settings, agents));
    let button = ReButton::new((label, icons::ARROW_RIGHT)).primary();
    let clicked = ui
        .add_enabled_ui(ready.is_ok(), |ui| {
            ui.add_sized([ui.available_width(), 0.0], button).clicked()
        })
        .inner;
    if clicked {
        next_screen = Screen::Chat;
    }
    if let Err(problem) = ready {
        ui.error_label(problem);
    }

    next_screen
}

/// One card per known agent, plus one for a custom command. Clicking a card selects it.
fn agent_cards_ui(ui: &mut egui::Ui, settings: &mut AgentSettings, agents: &[AgentEntry]) {
    let tokens = ui.tokens();
    let card_frame = egui::Frame::new()
        .inner_margin(CARD_INNER_MARGIN)
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.normal_corner_radius());
    let selected_frame = card_frame
        .fill(tokens.faint_bg_color)
        .stroke(tokens.focus_outline_stroke);

    let custom_index = agents.len();
    let selected_index = if settings.profile_id == AgentSettings::CUSTOM_PROFILE_ID {
        Some(custom_index)
    } else {
        agents
            .iter()
            .position(|agent| agent.profile.id == settings.profile_id)
    };
    let is_selected = |index: usize| selected_index == Some(index);

    let items = (0..=custom_index)
        .map(|index| CardLayoutItem {
            frame: is_selected(index).then_some(selected_frame),
            min_width: CARD_MIN_WIDTH,
            clickable: None,
        })
        .collect();

    let mut clicked = None;
    let card_clicked = CardLayout::new(items, card_frame)
        .clickable()
        .hover_stroke(tokens.card_hover_stroke)
        .show(ui, |ui, index, _hovered| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let selected = is_selected(index);
                if let Some(agent) = agents.get(index) {
                    if agent_card_ui(ui, agent, selected) {
                        clicked = Some(index);
                    }
                } else if custom_agent_card_ui(ui, settings, selected) {
                    clicked = Some(index);
                }
            });
        });

    if let Some(index) = card_clicked.or(clicked) {
        settings.profile_id = if index == custom_index {
            AgentSettings::CUSTOM_PROFILE_ID.to_owned()
        } else {
            agents[index].profile.id.clone()
        };
    }
}

/// Returns `true` if the radio button was clicked.
fn agent_card_ui(ui: &mut egui::Ui, agent: &AgentEntry, selected: bool) -> bool {
    let AgentEntry {
        profile,
        executable,
    } = agent;
    let tokens = ui.tokens();
    let installed = executable.is_some();

    let name = RichText::new(&profile.name).strong();
    let name = if installed {
        name
    } else {
        name.color(tokens.text_subdued)
    };
    let radio_clicked = ui.radio(selected, name).clicked();

    ui.horizontal(|ui| {
        if let Some(path) = executable {
            ui.label(RichText::new("Installed").color(tokens.success_text_color))
                .on_hover_text(path.display().to_string());
        } else {
            ui.weak("Not found ·");
            ui.re_hyperlink("Install", &profile.url, true)
                .on_hover_text(&profile.install_hint);
        }
    });

    ui.response().on_hover_text(&profile.description);
    radio_clicked
}

/// Returns `true` if the radio button was clicked, or the command edited.
fn custom_agent_card_ui(ui: &mut egui::Ui, settings: &mut AgentSettings, selected: bool) -> bool {
    let radio_clicked = ui
        .radio(selected, RichText::new("Custom command").strong())
        .on_hover_text(
            "Any command that speaks ACP over stdio. Leading NAME=value pairs set environment variables.",
        )
        .clicked();
    let edited = ui
        .add_sized(
            [ui.available_width(), ui.spacing().interact_size.y],
            ReTextEdit::singleline(&mut settings.custom_command_line).hint_text("my-agent --acp"),
        )
        .changed();
    radio_clicked || edited
}

/// What the user is signing up for before they hand the panel their agent.
///
/// One line only: what the agent does with their machine belongs next to the agent they pick.
fn warning_ui(ui: &mut egui::Ui) {
    Alert::info().show(ui, |ui| {
        // `show` hugs its content; the panel's blocks all run full-bleed.
        ui.set_min_width(ui.available_width());
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let text = re_ui::egui_ext::concat_rich_text(
            ui.style(),
            [
                RichText::new("Experimental.").strong(),
                RichText::new(" Expect rough edges."),
            ],
        );
        ui.label(text);
    });
}

/// The prompt-sharing opt-out, and one line on what the current setting means.
///
/// Both states get the same weight: sharing is the default, so it must not read as a hazard.
/// The particulars stay in the tooltip that the checkbox and "What is shared?" share.
fn prompt_analytics_ui(ui: &mut egui::Ui, share_redacted_prompts: &mut bool) {
    ui.scope(|ui| {
        // The consequence line belongs to the checkbox above it, closer than the panel's rhythm.
        ui.spacing_mut().item_spacing.y = 2.0;

        ui.re_checkbox(share_redacted_prompts, "Share redacted prompts with Rerun")
            .on_hover_ui(analytics_details_ui);

        let consequence = if *share_redacted_prompts {
            "Sent after a second agent redacts them."
        } else {
            // Not "stay on this machine": the agent still sends them to its model provider.
            "Prompts are not shared with Rerun."
        };
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
            ui.weak(consequence);
            analytics_details_button(ui);
        });
    });
}

/// The width the analytics details wrap at, in both the menu and the tooltip.
const ANALYTICS_DETAILS_WIDTH: f32 = 340.0;

/// "What is shared?": the particulars, on hover and on click.
///
/// Clickable as well as hoverable, like [`re_ui::UiExt::help_button`]: hover alone cannot be
/// reached without a pointer, and it gives the reader no way to keep the text open.
fn analytics_details_button(ui: &mut egui::Ui) -> egui::Response {
    let label = RichText::new("What is shared?")
        .underline()
        .color(ui.visuals().weak_text_color());
    let button = egui::Button::new(label).frame(false);

    let (response, menu) =
        egui::containers::menu::MenuButton::from_button(button).ui(ui, analytics_details_ui);

    if menu.is_some() {
        // Don't stack a tooltip on top of the open menu.
        response
    } else {
        response.on_hover_ui(analytics_details_ui)
    }
}

fn analytics_details_ui(ui: &mut egui::Ui) {
    ui.set_max_width(ANALYTICS_DETAILS_WIDTH);
    ui.label(ANALYTICS_DETAILS);
}

/// What the start button calls the selected agent.
fn selected_agent_name(settings: &AgentSettings, agents: &[AgentEntry]) -> String {
    agents
        .iter()
        .find(|agent| agent.profile.id == settings.profile_id)
        .map_or_else(|| "agent".to_owned(), |agent| agent.profile.name.clone())
}

/// Whether the selected agent can be started, or why not.
fn selected_agent_ready(settings: &AgentSettings, agents: &[AgentEntry]) -> Result<(), String> {
    if settings.profile_id == AgentSettings::CUSTOM_PROFILE_ID {
        if settings.custom_command_line.trim().is_empty() {
            return Err("Enter the command that starts your agent".to_owned());
        }
        return Ok(());
    }

    let Some(agent) = agents
        .iter()
        .find(|agent| agent.profile.id == settings.profile_id)
    else {
        return Err("Select an agent".to_owned());
    };
    if agent.executable.is_none() {
        return Err(format!(
            "{} is not installed. {}",
            agent.profile.name, agent.profile.install_hint
        ));
    }
    Ok(())
}

fn mcp_servers_ui(ui: &mut egui::Ui, servers: &mut Vec<McpServerConfig>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("MCP servers").strong());
        if ui
            .small_icon_button(&icons::ADD, "Add MCP server")
            .on_hover_text("Add MCP server")
            .clicked()
        {
            servers.push(McpServerConfig {
                name: "rerun".to_owned(),
                command_line: "rerun viewer-mcp".to_owned(),
                enabled: true,
            });
        }
    });
    ui.weak("Given to the agent when the session starts. `rerun viewer-mcp` lets it see and drive a running Rerun Viewer.");

    let mut remove = None;
    for (index, server) in servers.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let height = ui.spacing().interact_size.y;
            ui.re_checkbox(&mut server.enabled, "");
            ui.add_sized(
                [120.0, height],
                ReTextEdit::singleline(&mut server.name).hint_text("name"),
            )
            .on_hover_text("Name the agent sees this server under");

            // Right to left, so the remove button stays put and the command gets the rest.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_icon_button(&icons::REMOVE, "Remove")
                    .on_hover_text("Remove")
                    .clicked()
                {
                    remove = Some(index);
                }
                ui.add_sized(
                    [ui.available_width(), height],
                    ReTextEdit::singleline(&mut server.command_line).hint_text("command args…"),
                );
            });
        });
    }
    if let Some(index) = remove {
        servers.remove(index);
    }
}
