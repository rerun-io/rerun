//! Keyboard behavior of the chat composer.

use std::path::PathBuf;

use egui::Role;
use egui::Vec2;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_agent::acp::schema::v1::{
    AvailableCommand, AvailableCommandsUpdate, CurrentModeUpdate, PermissionOption,
    PermissionOptionKind, RequestPermissionRequest, SessionId, SessionMode, SessionModeState,
    SessionUpdate, ToolCallUpdate, ToolCallUpdateFields,
};
use re_agent::{AgentEntry, AgentEvent, AgentProfile, AgentProfileId, AgentSettings};
use re_agent_ui::AgentPanel;

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0);

/// Checks that approval shortcuts only answer permissions while the chat input has focus.
#[test]
fn permission_shortcuts_require_chat_input_focus() {
    for key in [egui::Key::Enter, egui::Key::Escape] {
        let mut panel = ready_panel();
        let session = panel.session_mut();
        session.begin_test_turn();
        session.push_test_permission_request(RequestPermissionRequest::new(
            SessionId::new("test-session"),
            ToolCallUpdate::new("tool-1", ToolCallUpdateFields::new()),
            vec![
                PermissionOption::new("allow", "Allow once", PermissionOptionKind::AllowOnce),
                PermissionOption::new("reject", "Reject once", PermissionOptionKind::RejectOnce),
            ],
        ));
        let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
            .build_ui_state(
                |ui, (panel, text): &mut (AgentPanel, String)| {
                    panel.ui(ui);
                    ui.text_edit_singleline(text).accessible_name("Other input");
                },
                (panel, String::new()),
            );
        harness.run_steps(3);
        harness.get_by_label("Other input").focus();
        harness.run_steps(2);
        harness.key_press(key);
        harness.run_steps(3);
        assert_eq!(
            harness.state().0.session().pending_permissions().len(),
            1,
            "{key:?} outside the chat must leave the permission pending",
        );

        harness.get_by_role(Role::MultilineTextInput).focus();
        harness.run_steps(2);
        harness.key_press(key);
        harness.run_steps(3);
        assert_eq!(
            harness.state().0.session().pending_permissions().len(),
            0,
            "{key:?} in the chat must answer the permission",
        );
    }
}

pub fn ready_panel() -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            executable: Some(PathBuf::from("/usr/local/bin").join(&profile.command)),
            profile,
        })
        .collect();
    let settings = AgentSettings {
        profile_id: AgentProfileId::new("claude"),
        ..Default::default()
    };
    let mut panel = AgentPanel::with_agents(settings, None, agents, Vec::new());
    panel.set_example_prompts(crate::example_prompts());
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
            vec![
                SessionMode::new("default", "Manual"),
                SessionMode::new("acceptEdits", "Accept edits"),
            ],
        )),
        config_options: Vec::new(),
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
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run_steps(2);

    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("/");
    harness.run_steps(3);

    let input = harness.get_by_role(Role::MultilineTextInput);
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
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run_steps(2);

    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("hi");
    harness.run_steps(2);

    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);

    let input = harness.get_by_role(Role::MultilineTextInput);
    assert!(input.accesskit_node().is_focused(), "input lost focus");
    assert_eq!(input.accesskit_node().value(), Some("hi".into()));
}

/// Opening the host panel asks the chat input to take keyboard focus.
#[test]
fn requested_input_focus_is_applied() {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run_steps(2);

    harness.get_by_value("Manual").focus();
    harness.run_steps(2);
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert!(!input.accesskit_node().is_focused());

    harness.state_mut().request_input_focus();
    harness.run_steps(2);
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert!(input.accesskit_node().is_focused());
}

/// Picking a mode in the footer dropdown asks the agent to switch.
#[test]
fn mode_picker_requests_the_clicked_mode() {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::CurrentModeUpdate(
            CurrentModeUpdate::new("default"),
        )));
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);

    harness.get_by_value("Manual").click();
    harness.run_steps(2);
    harness.get_by_label("Accept edits").click();
    harness.run_steps(2);

    let session = harness.state().session();
    assert_eq!(
        session.requested_mode().map(|id| id.0.as_ref()),
        Some("acceptEdits")
    );
    assert_eq!(
        harness
            .state()
            .settings()
            .preferred_mode
            .as_ref()
            .map(|mode| mode.0.as_ref()),
        Some("acceptEdits")
    );
}

/// Enter while the agent is busy queues the message. Escape stops the agent and hands the
/// queued text back to the input.
#[test]
fn enter_while_busy_queues_and_escape_takes_it_back() {
    let mut panel = ready_panel();
    panel.session_mut().begin_test_turn();
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);

    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("also fix the tests");
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);

    let session = harness.state().session();
    assert_eq!(session.queued_prompts().len(), 1);
    harness.get_by_label("also fix the tests");
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert_eq!(input.accesskit_node().value(), Some(String::new()));

    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);

    let session = harness.state().session();
    assert!(session.queued_prompts().is_empty());
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert_eq!(
        input.accesskit_node().value(),
        Some("also fix the tests".into())
    );
    assert!(input.accesskit_node().is_focused());
}

/// The send button is only enabled while there is something to send.
#[test]
fn send_button_needs_input() {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            ready_panel(),
        );
    harness.run_steps(2);
    assert!(harness.get_by_label("Send").accesskit_node().is_disabled());

    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("  ");
    harness.run_steps(2);
    assert!(harness.get_by_label("Send").accesskit_node().is_disabled());

    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("hi");
    harness.run_steps(2);
    assert!(!harness.get_by_label("Send").accesskit_node().is_disabled());
}

/// The stop button does what Escape does: it hands queued prompts back to the input.
#[test]
fn stop_button_takes_queued_prompts_back() {
    let mut panel = ready_panel();
    let session = panel.session_mut();
    session.begin_test_turn();
    assert!(session.send_prompt("also fix the tests".into()));
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);

    harness.get_by_label("Stop").click();
    harness.run_steps(3);

    let session = harness.state().session();
    assert!(session.queued_prompts().is_empty());
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert_eq!(
        input.accesskit_node().value(),
        Some("also fix the tests".into())
    );
}

/// An example prompt goes straight to the agent, leaving what was typed alone.
#[test]
fn clicking_an_example_prompt_sends_it() {
    let mut panel = ready_panel();
    // Busy, so the prompt waits in the session's queue, where the test can see it.
    panel.session_mut().begin_test_turn();
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);
    let input = harness.get_by_role(Role::MultilineTextInput);
    input.focus();
    input.type_text("draft");
    harness.run_steps(2);

    harness.get_by_label("Example prompt one").click();
    harness.run_steps(3);

    let session = harness.state().session();
    assert_eq!(
        session
            .queued_prompts()
            .iter()
            .map(|prompt| prompt.text.as_str())
            .collect::<Vec<_>>(),
        ["Example prompt one"]
    );
    let input = harness.get_by_role(Role::MultilineTextInput);
    assert_eq!(input.accesskit_node().value(), Some("draft".into()));
    assert!(input.accesskit_node().is_focused());
}
