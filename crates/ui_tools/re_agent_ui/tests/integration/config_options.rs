//! Generic session settings and profile-scoped selection persistence.

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent::acp::schema::v1::{
    ConfigOptionUpdate, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectGroup,
    SessionConfigSelectOption, SessionConfigValueId, SessionUpdate,
};
use re_agent::{AgentEvent, AgentProfileId, AgentSettings, ModelApiProvider};

/// Checks that config-only modes and uncategorized grouped options can be selected and restored.
#[test]
fn config_only_modes_and_uncategorized_options_are_selectable() {
    let mut panel = super::chat_input::ready_panel_for_profile(AgentProfileId::new("opencode"));
    panel
        .session_mut()
        .handle_event(AgentEvent::SessionStarted {
            session_id: "config-only".into(),
            origin: re_agent::SessionOrigin::New,
            config_options: vec![
                SessionConfigOption::select(
                    "mode",
                    "Session mode",
                    "build",
                    vec![
                        SessionConfigSelectOption::new("build", "Build"),
                        SessionConfigSelectOption::new("plan", "Plan"),
                    ],
                )
                .category(SessionConfigOptionCategory::Mode),
                SessionConfigOption::select(
                    "answer_language",
                    "Answer language",
                    "en",
                    vec![SessionConfigSelectGroup::new(
                        "languages",
                        "Languages",
                        vec![
                            SessionConfigSelectOption::new("en", "English"),
                            SessionConfigSelectOption::new("fr", "French"),
                        ],
                    )],
                ),
                SessionConfigOption::select(
                    "model",
                    "Model",
                    "luna",
                    vec![SessionConfigSelectOption::new("luna", "Luna")],
                )
                .category(SessionConfigOptionCategory::Model),
            ],
        });
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(|ui, panel| panel.ui(ui), panel);
    harness.run_steps(3);
    harness.get_by_label("Luna Build");
    assert!(harness.query_by_label_contains("English").is_none());
    super::open_settings_submenu(&mut harness, "Build", "Session mode");
    harness.get_by_label("Plan").click();
    harness.run_steps(2);
    super::open_settings_submenu(&mut harness, "Build", "Answer language");
    harness.snapshot("chat_generic_config_options");
    harness.get_by_label("French").click();
    harness.run_steps(2);
    let saved = serde_json::to_string(harness.state().settings()).expect("serialize settings");
    let restored: AgentSettings = serde_json::from_str(&saved).expect("restore settings");
    let preferences = restored
        .preferences(&AgentProfileId::new("opencode"))
        .expect("preferences");
    assert_eq!(
        preferences.config_options,
        [
            ("mode".into(), "plan".into()),
            ("answer_language".into(), "fr".into()),
        ]
        .into()
    );

    harness
        .state_mut()
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
            ConfigOptionUpdate::new(vec![
                SessionConfigOption::select(
                    "mode",
                    "Session mode",
                    "plan",
                    vec![SessionConfigSelectOption::new("plan", "Plan")],
                )
                .category(SessionConfigOptionCategory::Mode),
            ]),
        )));
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("English").is_none());
    harness.get_by_label("Plan");
}

/// Checks that native booleans render as checkboxes and persist as boolean values.
#[test]
fn boolean_options_use_checkboxes() {
    let mut panel = super::chat_input::ready_panel_for_profile(AgentProfileId::new("opencode"));
    panel
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
            ConfigOptionUpdate::new(vec![
                SessionConfigOption::boolean("auto_compact", "Auto compact", true)
                    .description("Compact the conversation when it gets long"),
            ]),
        )));
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(|ui, panel| panel.ui(ui), panel);
    harness.run_steps(3);
    harness.get_by_label("Settings").click();
    harness.run_steps(2);
    harness.snapshot("chat_boolean_config_option");
    harness.get_by_label("Auto compact").click();
    harness.run_steps(2);

    let saved = serde_json::to_value(harness.state().settings()).expect("save settings");
    assert_eq!(
        saved["preferences"]["opencode"]["config_options"]["auto_compact"],
        serde_json::json!({"type": "boolean", "value": false})
    );
    let restored: AgentSettings = serde_json::from_value(saved).expect("restore settings");
    assert_eq!(
        restored
            .preferences(&AgentProfileId::new("opencode"))
            .and_then(|preferences| preferences.get("auto_compact"))
            .and_then(|value| value.as_bool()),
        Some(false)
    );
}

/// Checks that model API selections persist without leaking into other providers or agent profiles.
#[test]
fn model_api_selections_are_scoped_to_the_provider() {
    let mut panel =
        super::chat_input::ready_panel_for_profile(ModelApiProvider::Anthropic.profile_id());
    panel
        .session_mut()
        .handle_event(AgentEvent::SessionStarted {
            session_id: "model-api".into(),
            origin: re_agent::SessionOrigin::New,
            config_options: vec![
                SessionConfigOption::select(
                    "model",
                    "Model",
                    "opus",
                    vec![
                        SessionConfigSelectOption::new("opus", "Opus"),
                        SessionConfigSelectOption::new("sonnet", "Sonnet"),
                    ],
                )
                .category(SessionConfigOptionCategory::Model),
            ],
        });
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(|ui, panel| panel.ui(ui), panel);
    harness.run_steps(3);
    super::open_settings_submenu(&mut harness, "Opus", "Model");
    harness.get_by_label("Sonnet").click();
    harness.run_steps(2);
    let saved = serde_json::to_value(harness.state().settings()).expect("save settings");
    let restored: AgentSettings = serde_json::from_value(saved).expect("restore settings");
    assert_eq!(
        restored
            .preferences(&ModelApiProvider::Anthropic.profile_id())
            .and_then(|preferences| preferences.model()),
        Some(&SessionConfigValueId::new("sonnet"))
    );
    assert!(
        restored
            .preferences(&ModelApiProvider::OpenAi.profile_id())
            .is_none()
    );
    assert!(
        restored
            .preferences(&AgentProfileId::new("claude"))
            .is_none()
    );
}
