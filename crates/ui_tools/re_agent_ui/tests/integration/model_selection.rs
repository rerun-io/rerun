use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent::acp::schema::v1::{
    ConfigOptionUpdate, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectGroup,
    SessionConfigSelectOption, SessionConfigValueId, SessionUpdate,
};
use re_agent::{
    AgentEntry, AgentEvent, AgentLaunch, AgentProfile, AgentProfileId, AgentSettings,
    SessionContext,
};
use re_agent_ui::AgentPanel;

use super::chat_input::ready_panel;

/// The model that `panel`'s agent harness would be started with.
fn launched_model(panel: &AgentPanel) -> Option<SessionConfigValueId> {
    match panel.agent_launch(&SessionContext::default()) {
        Ok(AgentLaunch::AgentHarness(config)) => config.preferred_model,
        other => panic!("expected an agent harness, got {other:?}"),
    }
}

#[test]
fn selected_model_is_preserved_for_new_conversations_and_after_settings_reload() {
    let mut panel = ready_panel();
    // A conversation of its own, which keeps its agent when another one is selected.
    panel
        .session_mut()
        .transcript_mut()
        .push_user("Hello".into());
    panel.settings_mut().preferred_models.insert(
        AgentProfileId::new("codex"),
        SessionConfigValueId::new("gpt-5.6-high"),
    );
    panel
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
            ConfigOptionUpdate::new(vec![
                SessionConfigOption::select(
                    "model",
                    "Model",
                    "opus",
                    vec![SessionConfigSelectGroup::new(
                        "anthropic",
                        "Anthropic",
                        vec![
                            SessionConfigSelectOption::new("opus", "Opus 5.5"),
                            SessionConfigSelectOption::new("opus-fast", "Opus 5.5 Fast"),
                            SessionConfigSelectOption::new("opus-thinking", "Opus 5.5 Thinking"),
                        ],
                    )],
                )
                .category(SessionConfigOptionCategory::Model),
            ]),
        )));
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(
        |ui, panel: &mut AgentPanel| {
            panel.ui(ui);
        },
        panel,
    );
    harness.run_steps(3);
    harness.get_by_value("Opus 5.5").click();
    harness.run_steps(2);
    harness.get_by_label("Opus 5.5 Fast").click();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .settings()
            .preferred_models
            .get(&AgentProfileId::new("claude")),
        Some(&SessionConfigValueId::new("opus-fast"))
    );
    assert_eq!(
        launched_model(harness.state()),
        Some(SessionConfigValueId::new("opus-fast"))
    );

    harness.state_mut().settings_mut().profile_id = AgentProfileId::new("codex");
    harness.get_by_value("Opus 5.5").click();
    harness.run_steps(2);
    harness.get_by_label("Opus 5.5 Thinking").click();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .settings()
            .preferred_models
            .get(&AgentProfileId::new("claude")),
        Some(&SessionConfigValueId::new("opus-thinking"))
    );
    assert_eq!(
        harness
            .state()
            .settings()
            .preferred_models
            .get(&AgentProfileId::new("codex")),
        Some(&SessionConfigValueId::new("gpt-5.6-high"))
    );

    harness.state_mut().new_conversation();
    assert_eq!(
        launched_model(harness.state()),
        Some(SessionConfigValueId::new("gpt-5.6-high"))
    );
    let saved = serde_json::to_string(harness.state().settings()).expect("save settings");
    let restored: AgentSettings = serde_json::from_str(&saved).expect("reload settings");
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            profile,
            executable: None,
        })
        .collect();
    let mut reopened = AgentPanel::with_agents(restored, None, agents, Vec::new());
    assert_eq!(
        launched_model(&reopened),
        Some(SessionConfigValueId::new("gpt-5.6-high"))
    );
    reopened.settings_mut().profile_id = AgentProfileId::new("claude");
    assert_eq!(
        launched_model(&reopened),
        Some(SessionConfigValueId::new("opus-thinking"))
    );
}
