//! Moving between the conversation list and the conversations in it.

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
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
    harness
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
