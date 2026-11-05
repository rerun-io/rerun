//! The setup screen: pick an agent, a working directory, MCP servers, and options.

use egui::RichText;
use re_ui::{ButtonExt as _, UiExt as _, icons};

use super::Screen;
use re_agent::AgentEntry;
use re_agent::{AgentProfileId, AgentSettings, HostedTools, ModelApiProvider};

#[cfg(agent_harness_backend)]
mod agent_harness;
mod prompt_sharing;

/// Where the selected model API's key comes from.
enum ApiKeySource<'a> {
    /// The web viewer: the user pastes it into this string.
    #[cfg_attr(
        all(agent_harness_backend, not(test)),
        expect(dead_code, reason = "only the web viewer takes typed keys")
    )]
    Typed(&'a mut String),

    /// The desktop viewer: read from the environment, if `found`.
    #[cfg_attr(
        not(agent_harness_backend),
        expect(dead_code, reason = "the web viewer has no environment")
    )]
    Environment { found: bool },
}

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
                next_screen = cfg_select! {
                    agent_harness_backend => setup_ui_inner(
                        ui,
                        settings,
                        agents,
                        available_model_apis,
                        agent_harness::agent_choice_ui,
                    ),
                    _ => setup_ui_inner(ui, settings, agents, available_model_apis, web_choice_ui),
                };
            });
        });

    next_screen
}

/// `choice_ui` picks the agent: on the desktop among agent harnesses and model APIs, on the web
/// among model APIs only.
fn setup_ui_inner(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    agents: &mut [AgentEntry],
    available_model_apis: &[ModelApiProvider],
    choice_ui: impl FnOnce(&mut egui::Ui, &mut AgentSettings, &mut [AgentEntry], &[ModelApiProvider]),
) -> Screen {
    let mut next_screen = Screen::Setup;
    let available_model_apis = &model_apis_with_key(settings, available_model_apis);

    ui.spacing_mut().item_spacing.y = 8.0;

    choice_ui(ui, settings, agents, available_model_apis);

    ui.add_space(4.0);
    let ready = selected_agent_ready(settings, agents, available_model_apis);
    // The provider toggle already names a model API.
    let label = if settings.profile_id.model_api_provider().is_some() {
        "Start".to_owned()
    } else {
        format!(
            "Start {}",
            selected_agent_name(settings, agents).unwrap_or_else(|| "agent".to_owned())
        )
    };
    let button = egui::Button::new((label, icons::ARROW_RIGHT))
        .primary()
        .size_small();
    let clicked = ui
        .add_enabled_ui(ready.is_ok(), |ui| {
            ui.add_sized([ui.available_width(), 0.0], button).clicked()
        })
        .inner;
    if clicked {
        next_screen = Screen::Chat;
    }
    // A missing key is already shown next to the provider toggle.
    if let Err(problem) = ready
        && settings.profile_id.model_api_provider().is_none()
    {
        ui.error_label(problem);
    }

    ui.add_space(4.0);
    ui.separator();
    prompt_sharing::prompt_analytics_ui(ui, &mut settings.share_redacted_prompts);

    ui.add_space(4.0);
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
        hosted_tools_ui(ui, &mut settings.model_api_hosted_tools);
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

/// Checkboxes for the tools the model provider runs on its own servers.
fn hosted_tools_ui(ui: &mut egui::Ui, hosted_tools: &mut HostedTools) {
    let HostedTools {
        code_execution,
        web_fetch,
        web_search,
    } = hosted_tools;
    ui.add_space(8.0);
    ui.label(RichText::new("Model-API tools").strong())
        .on_hover_text(
            "Tools that the Anthropic or OpenAI API runs on its own servers, \
             not in the viewer.",
        );
    let openai_web_note = "OpenAI has one web tool that both searches and reads pages. \
                           It is on if either of these two options is on.";
    ui.checkbox(web_fetch, "Read web pages")
        .on_hover_text(openai_web_note);
    ui.checkbox(web_search, "Search the web")
        .on_hover_text(openai_web_note);
    ui.checkbox(code_execution, "Run code in a sandbox")
        .on_hover_text(
            "For math, statistics, and plots. The sandbox has no direct access to the viewer, \
             but the model can put anything from the conversation into its code. \
             The provider bills its use.",
        );
}

/// The web viewer has no agent harnesses, so it only picks a model API.
#[cfg(target_arch = "wasm32")]
fn web_choice_ui(
    ui: &mut egui::Ui,
    settings: &mut AgentSettings,
    _agents: &mut [AgentEntry],
    available_model_apis: &[ModelApiProvider],
) {
    let api_keys = &mut settings.api_keys;
    model_api_choice_ui(
        ui,
        &mut settings.profile_id,
        available_model_apis,
        |provider| ApiKeySource::Typed(api_keys.entry(provider.profile_id()).or_default()),
    );
}

/// The choice on the web: a model API, and nothing else.
#[cfg(any(target_arch = "wasm32", test))]
fn model_api_choice_ui<'a>(
    ui: &mut egui::Ui,
    profile_id: &mut AgentProfileId,
    available_model_apis: &[ModelApiProvider],
    key_source: impl FnOnce(ModelApiProvider) -> ApiKeySource<'a>,
) {
    ui.label(RichText::new("Connect a model").strong().size(15.0));
    model_api_key_ui(ui, profile_id, available_model_apis, key_source);
}

/// Pick a model-API provider, and show where its key comes from.
///
/// `available_model_apis` are the providers whose key was found.
fn model_api_key_ui<'a>(
    ui: &mut egui::Ui,
    profile_id: &mut AgentProfileId,
    available_model_apis: &[ModelApiProvider],
    key_source: impl FnOnce(ModelApiProvider) -> ApiKeySource<'a>,
) {
    let mut provider = profile_id
        .model_api_provider()
        .unwrap_or_else(|| default_model_api(available_model_apis));
    ui.selectable_toggle(|ui| {
        for option in ModelApiProvider::ALL {
            ui.selectable_value(&mut provider, option, option.name());
        }
    });
    *profile_id = provider.profile_id();

    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;

        match key_source(provider) {
            ApiKeySource::Typed(key) => {
                ui.add(
                    re_ui::text_edit::ReTextEdit::singleline(key)
                        .password(true)
                        .hint_text(format!("Paste your {} API key", provider.name()))
                        .desired_width(f32::INFINITY),
                );
                key_status_ui(ui, provider, true, |ui| {
                    ui.label(RichText::new("Stays in this browser.").small().weak())
                        .on_hover_text(format!(
                            "The key is kept in this browser's local storage, \
                             and sent only to {}.",
                            provider.name()
                        ));
                });
            }
            ApiKeySource::Environment { found } => {
                let key_var = provider.api_key_env_var();
                key_status_ui(ui, provider, !found, |ui| {
                    if found {
                        tag_ui(ui, "Key found").on_hover_text(format!("Read from {key_var}"));
                    } else {
                        ui.label(
                            RichText::new(format!("Set {key_var}, then restart"))
                                .small()
                                .weak(),
                        )
                        .on_hover_text(format!(
                            "Set {key_var} in the environment the viewer starts from, \
                             then restart it"
                        ));
                    }
                });
            }
        }
    });
}

/// A line with `status_ui` on the left, and optionally a link to get a key on the right.
fn key_status_ui(
    ui: &mut egui::Ui,
    provider: ModelApiProvider,
    show_link: bool,
    status_ui: impl FnOnce(&mut egui::Ui),
) {
    egui::containers::Sides::new().show(ui, status_ui, |ui| {
        if show_link {
            ui.re_hyperlink("Get a key", provider.api_key_url(), true)
                .on_hover_text(format!("Create an API key at {}", provider.api_key_url()));
        }
    });
}

/// The providers whose key is at hand: from the settings, or `detected` in the environment.
pub fn model_apis_with_key(
    settings: &AgentSettings,
    detected: &[ModelApiProvider],
) -> Vec<ModelApiProvider> {
    ModelApiProvider::ALL
        .into_iter()
        .filter(|provider| detected.contains(provider) || settings.api_key(*provider).is_some())
        .collect()
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
        return if model_apis_with_key(settings, available_model_apis).contains(&provider) {
            Ok(())
        } else {
            Err(re_agent::AgentLaunchError::missing_api_key(provider).to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The web layout, which the integration tests cannot reach since they run on the desktop.
    #[test]
    fn web_setup_screen() {
        let mut harness = re_ui::testing::new_harness(
            re_ui::testing::TestOptions::Gui,
            egui::vec2(crate::RECOMMENDED_WIDTH, 400.0),
        )
        .build_ui_state(
            |ui, (settings, key): &mut (AgentSettings, String)| {
                egui::Frame::new().inner_margin(12).show(ui, |ui| {
                    _ = setup_ui_inner(ui, settings, &mut [], &[], |ui, settings, _, _| {
                        model_api_choice_ui(ui, &mut settings.profile_id, &[], |_| {
                            ApiKeySource::Typed(key)
                        });
                    });
                });
            },
            (AgentSettings::default(), String::new()),
        );
        harness.fit_contents();
        harness.snapshot("setup_screen_web");
    }
}
