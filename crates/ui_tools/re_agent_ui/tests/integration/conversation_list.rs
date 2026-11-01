//! Moving between the conversation list and the conversations in it.

use egui::Vec2;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_agent_ui::AgentPanel;

use super::chat_input::ready_panel;

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0);

/// A panel with two conversations, showing the list.
fn harness() -> egui_kittest::Harness<'static, AgentPanel> {
    let mut panel = ready_panel();
    for prompt in ["First question", "Second question"] {
        panel
            .session_mut()
            .transcript_mut()
            .push_user(prompt.into());
        panel.poll_events();
        panel.new_conversation();
    }

    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);
    back_to_list(&mut harness);
    harness
}

/// Leaves the new conversation that the panel shows after the last one was started.
fn back_to_list(harness: &mut egui_kittest::Harness<'static, AgentPanel>) {
    harness.get_by_label("Back to conversations").click();
    harness.run_steps(2);
}

#[test]
fn clicking_a_row_opens_it_and_back_returns_to_the_list() {
    let mut harness = harness();
    assert!(harness.query_by_label("Back to conversations").is_none());

    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "First question")
        .click();
    harness.run_steps(2);
    assert_eq!(harness.state().transcript().items.len(), 1);

    harness.get_by_label("Back to conversations").click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Back to conversations").is_none());
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Second question");
}

/// The close button shows on hover, and removes the conversation from the list.
#[test]
fn closing_a_row_removes_it() {
    let mut harness = harness();

    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "First question")
        .hover();
    harness.run_steps(2);
    harness.get_by_label("Close conversation").click();
    harness.run_steps(2);

    assert!(harness.query_by_label("First question").is_none());
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Second question");
}

#[test]
fn the_add_button_opens_a_new_conversation() {
    let mut harness = harness();
    // With setup done, the draft offers an input instead of the setup screen.
    harness.state_mut().settings_mut().setup_done = true;
    harness.run_steps(2);
    harness.get_by_label("More options").focus();
    harness.run_steps(2);
    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    assert!(!input.accesskit_node().is_focused());

    harness.get_by_label("New conversation").click();
    harness.run_steps(2);

    harness.get_by_label("Back to conversations");
    let input = harness.get_by_role(egui::accesskit::Role::MultilineTextInput);
    assert!(input.accesskit_node().is_focused());
}

/// The running turn of a closed conversation still reaches the host, as aborted.
#[test]
fn closing_a_running_conversation_reports_its_turn() {
    let mut panel = ready_panel();
    panel
        .session_mut()
        .transcript_mut()
        .push_user("Running question".into());
    panel.session_mut().begin_test_turn();
    panel.poll_events();
    panel.new_conversation();

    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(2);
    back_to_list(&mut harness);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Running question")
        .hover();
    harness.run_steps(2);
    harness.get_by_label("Close conversation").click();
    harness.run_steps(2);

    let turns = harness.state_mut().take_finished_turns();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].outcome, re_agent::TurnOutcome::Aborted);
}

/// The draft has no row of its own, so its startup error shows above the list, with a way to
/// fix the setup.
#[test]
fn draft_startup_error_shows_above_the_list() {
    let mut harness = harness();
    harness
        .state_mut()
        .session_mut()
        .report_error("Working directory not found");
    harness.run_steps(2);

    harness.get_by_label("Working directory not found");
    harness.get_by_label("Agent setup…").click();
    harness.run_steps(2);
    harness.get_by_label("Choose your agent");
}
