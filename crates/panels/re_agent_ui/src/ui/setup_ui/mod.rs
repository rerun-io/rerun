//! The setup screen: pick an agent, a working directory, MCP servers, and options.

use egui::RichText;
use re_ui::alert::Alert;
use re_ui::{ReButton, UiExt as _, icons};

use super::Screen;
use re_agent::AgentEntry;
use re_agent::{AgentProfileId, AgentSettings, ModelApiProvider};

#[cfg(agent_harness_backend)]
mod agent_harness;

/// Returns [`Screen::Chat`] when the user clicked "OK", so the caller should start the agent.
///
/// `available_model_apis` are the providers whose API key was found.
#[must_use]
pub fn setup_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
    available_model_apis: &[ModelApiProvider],
) -> Screen {
    let mut next_screen = Screen::Setup;

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            egui::Frame::new().inner_margin(12).show(ui, |ui| {
                next_screen = setup_ui_inner(ui, settings, agents, available_model_apis);
            });
        });

    next_screen
}

fn setup_ui_inner(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
    available_model_apis: &[ModelApiProvider],
) -> Screen {
    let mut next_screen = Screen::Setup;

    ui.spacing_mut().item_spacing.y = 8.0;

    warning_ui(ui);

    ui.add_space(8.0);
    ui.label(RichText::new("Choose your agent").strong().size(15.0));

    cfg_select! {
        agent_harness_backend => {
            agent_harness::agent_choice_ui(ui, settings, agents, available_model_apis);
        }
        _ => {
            model_api_list_ui(ui, settings, available_model_apis);
        }
    }

    ui.add_space(8.0);
    let ready = selected_agent_ready(settings, agents, available_model_apis);
    let label = format!(
        "Start {}",
        selected_agent_name(settings, agents).unwrap_or_else(|| "agent".to_owned())
    );
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
        if let Some(provider) = settings.profile_id.model_api_provider() {
            ui.hyperlink_to(
                format!("Get an API key from {}", provider.name()),
                provider.api_key_url(),
            );
        }
    }

    ui.add_space(8.0);
    ui.collapsing_header("Advanced", false, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;

        #[cfg(agent_harness_backend)]
        {
            agent_harness::options_ui(ui, settings);
            ui.add_space(8.0);
        }
        ui.label(RichText::new("Options").strong());
        ui.checkbox(&mut settings.show_thoughts, "Show the agent's thinking");
        ui.checkbox(
            &mut settings.log_protocol,
            "Log every protocol message (for debugging)",
        );
    });

    next_screen
}

/// What picking the model-API card selects: the first provider whose key was found.
///
/// With no key found it still picks one, so the start button can say which variable to set.
pub fn default_model_api(available_model_apis: &[ModelApiProvider]) -> ModelApiProvider {
    available_model_apis
        .first()
        .copied()
        .unwrap_or(ModelApiProvider::ALL[0])
}

/// One radio button per model-API provider.
///
/// `available_model_apis` are the ones whose key was found.
fn model_api_list_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    available_model_apis: &[ModelApiProvider],
) {
    for provider in ModelApiProvider::ALL {
        let selected = provider.profile_id() == settings.profile_id;
        if model_api_row_ui(ui, provider, available_model_apis, selected).clicked() {
            settings.profile_id = provider.profile_id();
        }
    }
}

/// Returns the response of the radio button, which is disabled until the API key is set.
fn model_api_row_ui(
    ui: &mut egui::Ui,
    provider: ModelApiProvider,
    available_model_apis: &[ModelApiProvider],
    selected: bool,
) -> egui::Response {
    let key_var = provider.api_key_env_var();
    let has_key = available_model_apis.contains(&provider);
    ui.horizontal(|ui| {
        let radio = ui
            .add_enabled(
                has_key,
                egui::RadioButton::new(selected, format!("{} API", provider.name())),
            )
            .on_disabled_hover_text(format!(
                "Set {key_var} in the environment the viewer starts from, then restart it"
            ));
        if has_key {
            tag_ui(ui, "Key found").on_hover_text(format!("Read from {key_var}"));
        } else {
            ui.weak(format!("needs {key_var}"));
        }
        radio
    })
    .inner
}

/// A small pill next to an option, saying that it is ready to use.
fn tag_ui(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let tokens = ui.tokens();
    egui::WidgetAtom::new(RichText::new(text).small())
        .frame(
            egui::Frame::new()
                .fill(tokens.example_tag_bg_fill)
                .stroke(tokens.example_tag_stroke)
                .corner_radius(tokens.normal_corner_radius())
                .inner_margin(egui::Margin::symmetric(6, 1)),
        )
        .show(ui)
        .response
}

/// What the user is signing up for before they hand the panel their agent.
///
/// One line only: what the agent does with their machine belongs next to the agent they pick.
fn warning_ui(ui: &mut egui::Ui) {
    Alert::info().show(ui, |ui| {
        // `show` hugs its content; the panel's blocks all run full-bleed.
        ui.set_min_width(ui.available_width());
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let text = egui::WidgetText::concat(
            ui,
            [
                RichText::new("Experimental.").strong(),
                RichText::new(" Expect rough edges."),
            ],
        );
        ui.label(text);
    });
}

/// The name of the selected agent, e.g. "Claude Code" or "Anthropic API".
pub fn selected_agent_name(settings: &AgentSettings, agents: &[AgentEntry]) -> Option<String> {
    if let Some(provider) = settings.profile_id.model_api_provider() {
        return Some(format!("{} API", provider.name()));
    }
    agents
        .iter()
        .find(|agent| agent.profile.id == settings.profile_id)
        .map(|agent| agent.profile.name.clone())
}

/// Whether the selected agent can be started, or why not.
///
/// `available_model_apis` are the providers whose API key was found.
pub fn selected_agent_ready(
    settings: &AgentSettings,
    agents: &[AgentEntry],
    available_model_apis: &[ModelApiProvider],
) -> Result<(), String> {
    if settings.profile_id == AgentProfileId::CUSTOM {
        if settings.custom_command_line.trim().is_empty() {
            return Err("Enter the command that starts your agent".to_owned());
        }
        return Ok(());
    }

    if let Some(provider) = settings.profile_id.model_api_provider() {
        return if available_model_apis.contains(&provider) {
            Ok(())
        } else {
            Err(re_agent::AgentLaunchError::MissingApiKey { provider }.to_string())
        };
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
