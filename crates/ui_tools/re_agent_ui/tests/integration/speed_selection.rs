use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent::acp::schema::v1::{
    ConfigOptionUpdate, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionUpdate,
};
use re_agent::{
    AgentEntry, AgentEvent, AgentLaunch, AgentProfile, AgentProfileId, AgentSettings,
    SessionContext,
};
use re_agent_ui::AgentPanel;

use super::chat_input::ready_panel;

/// A speed choice persists under the conversation's profile and reaches the next launch.
#[test]
fn speed_selection_is_saved_for_the_conversations_agent_profile() {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .transcript_mut()
        .push_user("Hello".into());
    panel
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
            ConfigOptionUpdate::new(vec![
                SessionConfigOption::select(
                    "model",
                    "Model",
                    "gpt-6.1-sol",
                    vec![SessionConfigSelectOption::new("gpt-6.1-sol", "GPT-6.1 Sol")],
                )
                .category(SessionConfigOptionCategory::Model),
                SessionConfigOption::select(
                    "fast-mode",
                    "Speed",
                    "standard",
                    vec![
                        SessionConfigSelectOption::new("standard", "Standard"),
                        SessionConfigSelectOption::new("priority", "Fast"),
                        SessionConfigSelectOption::new("ultrafast", "Ultrafast"),
                    ],
                )
                .category(SessionConfigOptionCategory::ModelConfig),
            ]),
        )));
    panel.settings_mut().profile_id = AgentProfileId::new("codex");
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(|ui, panel| panel.ui(ui), panel);
    harness.run_steps(3);
    harness.get_by_value("Standard").click();
    harness.run_steps(2);
    harness.snapshot("chat_speed_menu");
    harness.get_by_label("Ultrafast").click();
    harness.run_steps(2);

    let saved = serde_json::to_value(harness.state().settings()).expect("save settings");
    assert_eq!(saved["preferences"]["claude"]["speed"], "ultrafast");
    assert!(saved["preferences"]["codex"].is_null());
    let mut restored: AgentSettings = serde_json::from_value(saved).expect("reload settings");
    restored.profile_id = AgentProfileId::new("claude");
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            profile,
            executable: None,
        })
        .collect();
    let reopened = AgentPanel::with_agents(restored, None, agents, Vec::new());
    let Ok(AgentLaunch::AgentHarness(config)) = reopened.agent_launch(&SessionContext::default())
    else {
        panic!("expected an agent harness");
    };
    assert_eq!(
        config
            .preferred_speed
            .as_ref()
            .map(|value| value.0.as_ref()),
        Some("ultrafast")
    );
}

/// Claude's on/off values appear as Fast/Standard and preserve their protocol values.
#[test]
fn claude_fast_toggle_is_shown_as_standard_and_fast() {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
            ConfigOptionUpdate::new(vec![
                SessionConfigOption::select(
                    "fast",
                    "Fast mode",
                    "off",
                    vec![
                        SessionConfigSelectOption::new("off", "Off"),
                        SessionConfigSelectOption::new("on", "On"),
                    ],
                )
                .category(SessionConfigOptionCategory::ModelConfig),
            ]),
        )));
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(|ui, panel| panel.ui(ui), panel);
    harness.run_steps(3);
    harness.get_by_value("Standard").click();
    harness.run_steps(2);
    harness.get_by_label("Fast").click();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .settings()
            .preferences(&AgentProfileId::new("claude"))
            .and_then(|preferences| preferences.speed.as_ref())
            .map(|speed| speed.0.as_ref()),
        Some("on")
    );
}
