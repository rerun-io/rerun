//! Copying the answer of a finished turn.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent::acp::schema::v1::{
    ContentBlock, ContentChunk, SessionId, SessionMode, SessionModeState, SessionUpdate,
    TextContent,
};
use re_agent::{AgentEntry, AgentEvent, AgentProfile, AgentProfileId, AgentSettings};
use re_agent_ui::AgentPanel;

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 400.0);

const ANSWER: &str = "The viewer has two saved redap servers.";

fn panel(turn_in_progress: bool) -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            executable: Some(PathBuf::from("/a/fake/path/bin").join(&profile.command)),
            profile,
        })
        .collect();
    let settings = AgentSettings {
        profile_id: AgentProfileId::new("claude"),
        ..Default::default()
    };
    let mut panel = AgentPanel::with_agents(settings, None, agents, Vec::new());
    panel.show_chat();

    let session = panel.session_mut();
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
    if turn_in_progress {
        session.begin_test_turn();
    }
    session.handle_event(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
        ContentChunk::new(ContentBlock::Text(TextContent::new(ANSWER.to_owned()))),
    )));
    crate::pin_entry_times(session);
    panel
}

fn harness(panel: AgentPanel) -> egui_kittest::Harness<'static, AgentPanel> {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);
    harness
}

#[test]
fn hovering_a_finished_turn_offers_to_copy_its_answer() {
    crate::use_utc();
    let mut harness = harness(panel(false));
    assert!(harness.query_by_label("Copy response").is_none());

    harness.get_by_label_contains(ANSWER).hover();
    harness.run_steps(2);
    harness.snapshot("copy_response");

    harness.get_by_label("Copy response").click();
    harness.run_steps(1);
    let copied: Vec<&str> = harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, [ANSWER]);
}

#[test]
fn there_is_nothing_to_copy_while_the_turn_runs() {
    let mut harness = harness(panel(true));
    harness.get_by_label_contains(ANSWER).hover();
    harness.run_steps(2);
    assert!(harness.query_by_label("Copy response").is_none());
}
