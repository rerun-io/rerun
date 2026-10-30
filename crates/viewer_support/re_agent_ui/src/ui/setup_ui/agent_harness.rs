//! The parts of the setup screen that only an agent harness needs: the agents on this machine,
//! a custom command, prompt sharing, and the working directory, environment, and MCP servers.

use egui::RichText;
use re_ui::text_edit::ReTextEdit;
use re_ui::{UiExt as _, icons};

use re_agent::{
    AgentEntry, AgentKind, AgentProfileId, AgentSettings, McpServerConfig, ModelApiProvider,
};

use super::{default_model_api, model_api_list_ui, tag_ui};

/// The two choice cards: an agent on this machine, or a model API.
pub fn agent_choice_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
    available_model_apis: &[ModelApiProvider],
) {
    let kind = settings.profile_id.kind();

    let local_selected = kind == AgentKind::AgentHarness;
    let picked_local = choice_card_ui(
        ui,
        local_selected,
        "A coding agent on this machine",
        "Uses your existing subscription. Can read local files and run tools.",
        |ui| local_agents_ui(ui, settings, agents),
    );
    if picked_local && !local_selected {
        settings.profile_id = default_local_agent(agents);
    }

    let model_api_selected = kind == AgentKind::ModelApi;
    let picked_model_api = choice_card_ui(
        ui,
        model_api_selected,
        "A model API with your own key",
        "Nothing to install. Pay per token. Reads the key from your environment.",
        |ui| model_api_list_ui(ui, settings, available_model_apis),
    );
    if picked_model_api && !model_api_selected {
        settings.profile_id = default_model_api(available_model_apis).profile_id();
    }
}

/// The working directory, environment, and MCP servers of an agent harness.
pub fn options_ui(ui: &mut egui::Ui, settings: &mut AgentSettings) {
    ui.label(RichText::new("Working directory").strong());
    let current_dir =
        std::env::current_dir().map_or_else(|_| String::new(), |dir| dir.display().to_string());
    ui.add(
        ReTextEdit::singleline(&mut settings.cwd)
            .hint_text(current_dir)
            .desired_width(f32::INFINITY),
    );
    ui.weak("The agent reads and edits files here. Point it at the project you want help with.");

    ui.add_space(8.0);
    ui.label(RichText::new("Environment variables").strong());
    ui.add(
        ReTextEdit::multiline(&mut settings.env_vars)
            .hint_text("CLAUDE_CONFIG_DIR=~/.claude-work")
            .desired_width(f32::INFINITY),
    );
    ui.weak("One NAME=value per line, set for the agent process.");

    ui.add_space(8.0);
    mcp_servers_ui(ui, &mut settings.mcp_servers);
}

/// A selectable card: a radio button with a title and a description.
///
/// The body is only shown while the card is selected.
/// Returns `true` if the radio button was clicked.
fn choice_card_ui(
    ui: &mut egui::Ui,
    selected: bool,
    title: &str,
    description: &str,
    add_body: impl FnOnce(&mut egui::Ui),
) -> bool {
    let stroke = if selected {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };

    egui::Frame::new()
        .stroke(stroke)
        .corner_radius(ui.tokens().normal_corner_radius())
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

            let clicked = ui.radio(selected, RichText::new(title).strong()).clicked();
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
                ui.weak(description);
            });

            if selected {
                ui.add_space(4.0);
                ui.indent(title, add_body);
            }

            clicked
        })
        .inner
}

/// The installed agents, with the others and a custom command behind a collapsing header.
fn local_agents_ui(ui: &mut egui::Ui, settings: &mut AgentSettings, agents: &mut [AgentEntry]) {
    let mut picked = None;
    for agent in agents.iter().filter(|agent| agent.executable.is_some()) {
        let selected = agent.profile.id == settings.profile_id;
        if agent_row_ui(ui, agent, selected).clicked() {
            picked = Some(agent.profile.id.clone());
        }
    }

    let custom_selected = settings.profile_id == AgentProfileId::CUSTOM;
    let any_installed = agents.iter().any(|agent| agent.executable.is_some());
    ui.collapsing_header(
        "Other agents and custom command",
        custom_selected || !any_installed,
        |ui| {
            for agent in agents.iter().filter(|agent| agent.executable.is_none()) {
                let selected = agent.profile.id == settings.profile_id;
                if agent_row_ui(ui, agent, selected).clicked() {
                    picked = Some(agent.profile.id.clone());
                }
            }
            if custom_agent_ui(ui, settings, custom_selected) {
                picked = Some(AgentProfileId::CUSTOM);
            }
        },
    );

    if let Some(profile_id) = picked {
        settings.profile_id = profile_id;
    }

    ui.horizontal(|ui| {
        ui.weak("Installed something new?");
        if ui.link("Re-check").clicked() {
            for agent in agents.iter_mut() {
                agent.refresh();
            }
        }
    });
}

/// What picking the local card selects: the first installed agent, else a custom command.
fn default_local_agent(agents: &[AgentEntry]) -> AgentProfileId {
    agents
        .iter()
        .find(|agent| agent.executable.is_some())
        .map_or(AgentProfileId::CUSTOM, |agent| agent.profile.id.clone())
}

/// Returns the response of the radio button.
fn agent_row_ui(ui: &mut egui::Ui, agent: &AgentEntry, selected: bool) -> egui::Response {
    let AgentEntry {
        profile,
        executable,
    } = agent;

    ui.horizontal(|ui| {
        let radio = ui
            .add_enabled(
                executable.is_some(),
                egui::RadioButton::new(selected, &profile.name),
            )
            .on_disabled_hover_text("Not installed");
        if let Some(path) = executable {
            tag_ui(ui, "Installed").on_hover_text(path.display().to_string());
        }
        radio
    })
    .inner
}

/// Returns `true` if the radio button was clicked, or the command field focused or edited.
fn custom_agent_ui(ui: &mut egui::Ui, settings: &mut AgentSettings, selected: bool) -> bool {
    let radio_clicked = ui
        .radio(selected, "Custom command")
        .on_hover_text(
            "Any command that speaks ACP over stdio. Leading NAME=value pairs set environment variables.",
        )
        .clicked();
    let field_used = ui
        .indent("custom_command", |ui| {
            let response = ui.add(
                ReTextEdit::singleline(&mut settings.custom_command_line)
                    .hint_text("my-agent --acp")
                    .desired_width(f32::INFINITY),
            );
            response.gained_focus() || response.changed()
        })
        .inner;
    radio_clicked || field_used
}

fn mcp_servers_ui(ui: &mut egui::Ui, servers: &mut Vec<McpServerConfig>) {
    egui::containers::Sides::new().show(
        ui,
        |ui| ui.label(RichText::new("MCP servers").strong()),
        |ui| {
            if ui
                .link(RichText::new("Add").underline())
                .on_hover_text("Add an MCP server")
                .clicked()
            {
                servers.push(McpServerConfig {
                    name: "rerun".to_owned(),
                    command_line: "rerun viewer-mcp".to_owned(),
                    enabled: true,
                });
            }
        },
    );
    ui.weak("Given to the agent when the session starts. `rerun viewer-mcp` lets it see and drive a running Rerun Viewer.");

    let mut remove = None;
    for (index, server) in servers.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            // Set first for centering
            ui.set_min_height(re_ui::DesignTokens::list_item_height(
                re_ui::ListItemDensity::Spacious,
            ));

            let response = ui
                .checkbox(&mut server.enabled, "")
                .on_hover_text("Give this server to the agent");
            let enabled = server.enabled;
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::Role::CheckBox,
                    true,
                    enabled,
                    "Give this server to the agent",
                )
            });
            ui.add(
                ReTextEdit::singleline(&mut server.name)
                    .hint_text("name")
                    .desired_width(120.0),
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
                ui.add(
                    ReTextEdit::singleline(&mut server.command_line)
                        .hint_text("command args…")
                        .desired_width(ui.available_width()),
                );
            });
        });
    }
    if let Some(index) = remove {
        servers.remove(index);
    }
}
