//! Keyboard behavior of the chat composer.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_agent_ui::acp::schema::v1::{
    AvailableCommand, AvailableCommandsUpdate, CurrentModeUpdate, SessionId, SessionMode,
    SessionModeState, SessionUpdate,
};
use re_agent_ui::{AgentEntry, AgentEvent, AgentPanel, AgentProfile, AgentSettings};

const SIZE: Vec2 = Vec2::new(600.0, 800.0);

fn ready_panel() -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            executable: Some(PathBuf::from("/usr/local/bin").join(&profile.command)),
            profile,
        })
        .collect();
    let settings = AgentSettings {
        profile_id: "claude".to_owned(),
        ..Default::default()
    };
    let mut panel = AgentPanel::with_agents(settings, agents);
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
            vec![
                SessionMode::new("default", "Manual"),
                SessionMode::new("acceptEdits", "Accept edits"),
            ],
        )),
    });
    session.handle_event(AgentEvent::Update(SessionUpdate::AvailableCommandsUpdate(
        AvailableCommandsUpdate::new(vec![AvailableCommand::new("help", "Show help")]),
    )));
    panel
}

/// Typing `/` opens the command list above the input. The input must keep keyboard focus.
#[test]
fn slash_command_popup_keeps_focus_in_the_input() {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run_steps(2);

    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    input.focus();
    input.type_text("/");
    harness.run_steps(3);

    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    assert!(input.accesskit_node().is_focused(), "input lost focus");
    assert_eq!(input.accesskit_node().value(), Some("/".into()));
    harness.get_by_label_contains("/help");
}

/// Escape is "stop the agent" while a turn runs, so it must not throw the input out of focus.
#[test]
fn escape_keeps_focus_in_the_input() {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run_steps(2);

    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    input.focus();
    input.type_text("hi");
    harness.run_steps(2);

    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);

    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    assert!(input.accesskit_node().is_focused(), "input lost focus");
    assert_eq!(input.accesskit_node().value(), Some("hi".into()));
}

/// Picking a mode in the footer dropdown asks the agent to switch.
#[test]
fn mode_picker_requests_the_clicked_mode() {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .expect("one conversation")
        .handle_event(AgentEvent::Update(SessionUpdate::CurrentModeUpdate(
            CurrentModeUpdate::new("default"),
        )));
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);

    harness.get_by_value("Manual").click();
    harness.run_steps(2);
    harness.get_by_label("Accept edits").click();
    harness.run_steps(2);

    let session = harness.state().session().expect("one conversation");
    assert_eq!(
        session.requested_mode().map(|id| id.0.as_ref()),
        Some("acceptEdits")
    );
}

/// Enter while the agent is busy queues the message. Escape stops the agent and hands the
/// queued text back to the input.
#[test]
fn enter_while_busy_queues_and_escape_takes_it_back() {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .expect("one conversation")
        .begin_test_turn();
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);

    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    input.focus();
    input.type_text("also fix the tests");
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);

    let session = harness.state().session().expect("one conversation");
    assert_eq!(session.queued_prompts().len(), 1);
    harness.get_by_label("also fix the tests");
    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    assert_eq!(input.accesskit_node().value(), Some(String::new()));

    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);

    let session = harness.state().session().expect("one conversation");
    assert!(session.queued_prompts().is_empty());
    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    assert_eq!(
        input.accesskit_node().value(),
        Some("also fix the tests".into())
    );
    assert!(input.accesskit_node().is_focused());
}
