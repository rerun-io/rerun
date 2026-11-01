//! "Self-improve" in the options menu, which only Rerun development builds turn on.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent::acp::schema::v1::{SessionId, SessionMode, SessionModeState};
use re_agent::{AgentEntry, AgentEvent, AgentProfile, AgentProfileId, AgentSettings};
use re_agent_ui::AgentPanel;

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0);

const SELF_IMPROVE_LABEL: &str = "Self-improve";

/// A panel past its setup screen, so that the options menu has all its entries.
///
/// The session is driven straight to "started", so the panel never launches the agent: an
/// `executable` merely has to be `Some` for the panel to count it as installed, and a launch
/// would make the snapshot depend on how fast the machine fails to run a made-up path.
fn panel(self_improve_available: bool) -> AgentPanel {
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
    panel.set_self_improve_available(self_improve_available);
    panel.show_chat();
    // Self-improve reviews the shown conversation, so the list does not offer it.
    panel.queue_prompt("Hello".to_owned());

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

fn open_menu(harness: &mut egui_kittest::Harness<'static, AgentPanel>) {
    harness.get_by_label("More options").click();
    harness.run_steps(2);
}

#[test]
fn self_improve_reports_only_the_frame_it_was_picked_in() {
    let mut harness = harness(panel(true));
    open_menu(&mut harness);
    assert!(!harness.state().self_improve_requested());

    // The badge colors say, without a tooltip, that this is not something every user has.
    harness.snapshot("self_improve");

    harness.get_by_label_contains(SELF_IMPROVE_LABEL).click();
    harness.run_steps(1);
    assert!(harness.state().self_improve_requested());

    // Otherwise the host would act on the same click again on the next frame.
    harness.run_steps(1);
    assert!(!harness.state().self_improve_requested());
}

#[test]
fn there_is_no_self_improve_unless_the_host_asks_for_it() {
    let mut harness = harness(panel(false));
    open_menu(&mut harness);
    assert!(
        harness
            .query_by_label_contains(SELF_IMPROVE_LABEL)
            .is_none()
    );
    assert!(!harness.state().self_improve_requested());
}

/// On the setup screen, the menu has nothing that needs a running chat, and a back button
/// leads to the conversation.
#[test]
fn setup_screen_menu_has_no_chat_actions() {
    let mut harness = harness(panel(true));
    harness.get_by_label("More options").click();
    harness.run_steps(2);
    harness.get_by_label("Agent setup…").click();
    harness.run_steps(2);

    harness.get_by_label("More options").click();
    harness.run_steps(2);
    harness.get_by_label_contains(SELF_IMPROVE_LABEL);
    assert!(harness.query_by_label("Agent setup…").is_none());
    assert!(
        harness
            .query_by_label("Clear session and restart agent")
            .is_none()
    );

    harness.get_by_label("Back to the conversation").click();
    harness.run_steps(2);
    harness.get_by_role(egui::Role::MultilineTextInput);
}

/// Picking a menu entry with the keyboard closes the menu, as a click does.
#[test]
fn keyboard_activation_closes_the_menu() {
    let mut harness = harness(panel(true));
    harness.get_by_label("More options").click();
    harness.run_steps(2);

    harness.get_by_label("Agent setup…").focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);

    harness.get_by_label("Back to the conversation");
    assert!(
        harness
            .query_by_label_contains(SELF_IMPROVE_LABEL)
            .is_none(),
        "the menu should close"
    );
}
