use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent_ui::acp::schema::v1::{
    ConfigOptionUpdate, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectGroup,
    SessionConfigSelectOption, SessionConfigValueId, SessionUpdate,
};
use re_agent_ui::{
    AgentEntry, AgentEvent, AgentPanel, AgentProfile, AgentSettings, SessionContext,
};

use super::chat_input::ready_panel;

#[test]
fn selected_model_is_preserved_for_new_tabs_and_after_settings_reload() {
    let mut panel = ready_panel();
    panel.settings_mut().preferred_models.insert(
        "codex".to_owned(),
        SessionConfigValueId::new("gpt-5.6-high"),
    );
    panel
        .session_mut()
        .expect("session")
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
        harness.state().settings().preferred_models.get("claude"),
        Some(&SessionConfigValueId::new("opus-fast"))
    );
    assert_eq!(
        harness
            .state()
            .launch_config(&SessionContext::default())
            .expect("launch config")
            .preferred_model,
        Some(SessionConfigValueId::new("opus-fast"))
    );

    harness.state_mut().settings_mut().profile_id = "codex".to_owned();
    harness.get_by_value("Opus 5.5").click();
    harness.run_steps(2);
    harness.get_by_label("Opus 5.5 Thinking").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().settings().preferred_models.get("claude"),
        Some(&SessionConfigValueId::new("opus-thinking"))
    );
    assert_eq!(
        harness.state().settings().preferred_models.get("codex"),
        Some(&SessionConfigValueId::new("gpt-5.6-high"))
    );

    harness.state_mut().new_conversation();
    assert_eq!(
        harness
            .state()
            .launch_config(&SessionContext::default())
            .expect("new tab launch config")
            .preferred_model,
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
    let mut reopened = AgentPanel::with_agents(restored, agents);
    assert_eq!(
        reopened
            .launch_config(&SessionContext::default())
            .expect("reopened launch config")
            .preferred_model,
        Some(SessionConfigValueId::new("gpt-5.6-high"))
    );
    reopened.settings_mut().profile_id = "claude".to_owned();
    assert_eq!(
        reopened
            .launch_config(&SessionContext::default())
            .expect("Claude launch config")
            .preferred_model,
        Some(SessionConfigValueId::new("opus-thinking"))
    );
}
