//! The feedback button in the tab bar.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent_ui::acp::schema::v1::{
    ContentBlock, ContentChunk, SessionId, SessionMode, SessionModeState, SessionUpdate,
    TextContent,
};
use re_agent_ui::{AgentEntry, AgentEvent, AgentPanel, AgentProfile, AgentSettings};

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 400.0);

const ANSWER: &str = "The viewer has two saved redap servers.";

fn panel(feedback_enabled: bool) -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            executable: Some(PathBuf::from("/a/fake/path/bin").join(&profile.command)),
            profile,
        })
        .collect();
    let settings = AgentSettings {
        profile_id: "claude".to_owned(),
        ..Default::default()
    };
    let mut panel = AgentPanel::with_agents(settings, agents);
    panel.set_feedback_enabled(feedback_enabled);
    panel.show_chat();

    let session = panel.session_mut().expect("one conversation");
    session.handle_event(AgentEvent::Initialized {
        agent_info: None,
        capabilities: Box::default(),
        auth_methods: Vec::new(),
    });
    session.handle_event(AgentEvent::SessionStarted {
        session_id: SessionId::new("test-session"),
        modes: Some(SessionModeState::new(
            "default",
            vec![SessionMode::new("default", "Manual")],
        )),
        config_options: Vec::new(),
    });
    session
        .transcript_mut()
        .push_user("What redap servers do we have?".into());
    session.handle_event(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
        ContentChunk::new(ContentBlock::Text(TextContent::new(ANSWER.to_owned()))),
    )));
    panel
}

fn harness(panel: AgentPanel) -> egui_kittest::Harness<'static, AgentPanel> {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);
    harness
}

#[test]
fn the_feedback_click_is_reported_only_in_the_frame_it_happened() {
    let mut harness = harness(panel(true));
    assert!(!harness.state().feedback_requested());
    harness.snapshot("feedback_button");

    harness.get_by_label("Feedback").click();
    harness.run_steps(1);
    assert!(harness.state().feedback_requested());

    harness.run_steps(1);
    assert!(!harness.state().feedback_requested());
}

#[test]
fn there_is_no_feedback_button_while_the_turn_runs() {
    let mut panel = panel(true);
    panel
        .session_mut()
        .expect("one conversation")
        .begin_test_turn();
    let harness = harness(panel);
    assert!(harness.query_by_label("Feedback").is_none());
}

#[test]
fn feedback_is_off_unless_the_host_enables_it() {
    let harness = harness(panel(false));
    assert!(harness.query_by_label("Feedback").is_none());
}
