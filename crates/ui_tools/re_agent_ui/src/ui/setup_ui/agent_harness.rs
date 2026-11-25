//! The parts of the setup screen that only an agent harness needs: the agents on this machine,
//! a custom command, prompt sharing, and the working directory, environment, and MCP servers.

use egui::RichText;
use re_ui::text_edit::ReTextEdit;
use re_ui::{UiExt as _, icons};

use re_agent::{
    AgentEntry, AgentInstall, AgentKind, AgentProfile, AgentProfileId, AgentSettings,
    McpServerConfig, McpStdioServer, ModelApiProvider,
};

use super::{ApiKeySource, default_model_api, model_api_key_ui, tag_ui};

/// The two choice cards: an agent on this machine, or a model API.
pub fn agent_choice_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
    available_model_apis: &[ModelApiProvider],
) {
    ui.label(RichText::new("Choose your agent").strong().size(15.0));

    let local_selected = settings.profile_id.kind() == AgentKind::AgentHarness;
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

    // A card's body writes `profile_id`, so each card must read the current one.
    let model_api_selected = settings.profile_id.kind() == AgentKind::ModelApi;
    let picked_model_api = choice_card_ui(
        ui,
        model_api_selected,
        "A model API with your own key",
        "Nothing to install. Pay per token. Reads the key from your environment.",
        |ui| {
            model_api_key_ui(
                ui,
                &mut settings.profile_id,
                available_model_apis,
                |provider| ApiKeySource::Environment {
                    found: available_model_apis.contains(&provider),
                },
            );
        },
    );
    if picked_model_api && !model_api_selected {
        settings.profile_id = default_model_api(available_model_apis).profile_id();
    }
}

/// The working directory, environment, and MCP servers of an agent harness.
/// `rerun_mcp_server` is the host's own MCP server, see [`re_agent::SessionContext::rerun_mcp_server`].
pub fn options_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    rerun_mcp_server: Option<&McpStdioServer>,
) {
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
    mcp_servers_ui(
        ui,
        rerun_mcp_server,
        &mut settings.rerun_mcp_enabled,
        &mut settings.mcp_servers,
    );
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

            let clicked = ui
                .re_radio(selected, RichText::new(title).strong())
                .clicked();
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

/// The agents found on this machine, with the others and a custom command behind a collapsing header.
fn local_agents_ui(ui: &mut egui::Ui, settings: &mut AgentSettings, agents: &mut [AgentEntry]) {
    let window_focused = ui.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::WindowFocused(true)))
    });
    if window_focused {
        for agent in agents.iter_mut() {
            agent.refresh();
        }
    }

    let found = |agent: &&AgentEntry| agent.install != AgentInstall::NotInstalled;
    let mut picked = None;
    for agent in agents.iter().filter(found) {
        let selected = agent.profile.id == settings.profile_id;
        if agent_row_ui(ui, agent, selected).clicked() {
            picked = Some(agent.profile.id.clone());
        }
    }

    let custom_selected = settings.profile_id == AgentProfileId::CUSTOM;
    let missing_selected = agents.iter().any(|agent| {
        agent.install == AgentInstall::NotInstalled && agent.profile.id == settings.profile_id
    });
    let any_found = agents.iter().any(|agent| found(&agent));
    ui.collapsing_header(
        "Other agents and custom command",
        custom_selected || missing_selected || !any_found,
        |ui| {
            for agent in agents
                .iter()
                .filter(|agent| agent.install == AgentInstall::NotInstalled)
            {
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

/// What picking the local card selects: the first agent that can start, else the first that
/// needs setup, else a custom command.
fn default_local_agent(agents: &[AgentEntry]) -> AgentProfileId {
    let first = |wanted: fn(&AgentInstall) -> bool| {
        agents
            .iter()
            .find(|agent| wanted(&agent.install))
            .map(|agent| agent.profile.id.clone())
    };
    first(|install| matches!(install, AgentInstall::Ready(_)))
        .or_else(|| first(|install| matches!(install, AgentInstall::NeedsCommand { .. })))
        .unwrap_or(AgentProfileId::CUSTOM)
}

/// Returns the response of the radio button.
fn agent_row_ui(ui: &mut egui::Ui, agent: &AgentEntry, selected: bool) -> egui::Response {
    let AgentEntry { profile, install } = agent;

    let radio = ui
        .horizontal(|ui| {
            let radio = ui.re_radio(selected, &profile.name);
            match install {
                AgentInstall::Ready(path) => {
                    tag_ui(ui, "Installed").on_hover_text(path.display().to_string());
                }
                AgentInstall::NeedsCommand { cli } => {
                    tag_ui(ui, "Needs setup").on_hover_text(cli.display().to_string());
                }
                AgentInstall::NotInstalled => {}
            }
            radio
        })
        .inner;
    let problem = match install {
        AgentInstall::Ready(_) => None,
        AgentInstall::NeedsCommand { .. } => Some(format!(
            "{} is installed, but the panel starts it with {}, which was not found.",
            profile.name, profile.command
        )),
        AgentInstall::NotInstalled => Some(format!("{} is not installed.", profile.name)),
    };
    if selected && let Some(problem) = problem {
        ui.indent(&profile.id, |ui| setup_steps_ui(ui, profile, problem));
    }
    radio
}

fn setup_steps_ui(ui: &mut egui::Ui, profile: &AgentProfile, problem: String) {
    egui::Frame::new()
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(ui.tokens().normal_corner_radius())
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(problem);
            ui.label(&profile.install_hint);
            ui.re_hyperlink("Learn more", &profile.url, true);
        });
}

/// Returns `true` if the radio button was clicked, or the command field focused or edited.
fn custom_agent_ui(ui: &mut egui::Ui, settings: &mut AgentSettings, selected: bool) -> bool {
    let radio_clicked = ui
        .re_radio(selected, "Custom command")
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

fn mcp_servers_ui(
    ui: &mut egui::Ui,
    rerun_mcp_server: Option<&McpStdioServer>,
    rerun_mcp_enabled: &mut bool,
    servers: &mut Vec<McpServerConfig>,
) {
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
                    name: String::new(),
                    command_line: String::new(),
                    enabled: true,
                });
            }
        },
    );
    ui.weak("Given to the agent when the session starts.");

    if let Some(server) = rerun_mcp_server {
        ui.horizontal(|ui| {
            ui.set_min_height(re_ui::DesignTokens::list_item_height(
                re_ui::ListItemDensity::Spacious,
            ));
            mcp_server_enabled_ui(ui, rerun_mcp_enabled);
            // The executable's name rather than its full path, which can be long.
            let executable = server.command.file_name().map_or_else(
                || server.command.to_string_lossy(),
                |name| name.to_string_lossy(),
            );
            let short_command = std::iter::chain(
                std::iter::once(executable.as_ref()),
                server.args.iter().map(String::as_str),
            )
            .collect::<Vec<_>>()
            .join(" ");
            ui.add(egui::Label::new(short_command).truncate())
                .on_hover_text(format!(
                    "`rerun mcp` is what lets the agent panel connect to this viewer and your \
                     catalog servers.\n\n{}",
                    server.command_line()
                ));
        });
    }

    let mut remove = None;
    for (index, server) in servers.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            // Set first for centering
            ui.set_min_height(re_ui::DesignTokens::list_item_height(
                re_ui::ListItemDensity::Spacious,
            ));

            mcp_server_enabled_ui(ui, &mut server.enabled);
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

fn mcp_server_enabled_ui(ui: &mut egui::Ui, enabled: &mut bool) {
    let response = ui
        .checkbox(enabled, "")
        .on_hover_text("Enable this MCP server");
    let is_enabled = *enabled;
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::CheckBox,
            true,
            is_enabled,
            "Enable this MCP server",
        )
    });
}
