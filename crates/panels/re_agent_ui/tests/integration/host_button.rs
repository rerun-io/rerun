//! The button a host adds to the tab bar for an action of its own.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent_ui::acp::schema::v1::{SessionId, SessionMode, SessionModeState};
use re_agent_ui::{AgentEntry, AgentEvent, AgentPanel, AgentProfile, AgentSettings, HostButton};

const SIZE: Vec2 = Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0);

const LABEL: &str = "Self-improve";

/// A panel past its setup screen, so that the tab bar the button lives on is the thing on screen.
///
/// The session is driven straight to "started", so the panel never launches the agent: an
/// `executable` merely has to be `Some` for the panel to count it as installed, and a launch
/// would make the snapshot depend on how fast the machine fails to run a made-up path.
fn panel(button: Option<HostButton>) -> AgentPanel {
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
    panel.set_host_button(button);
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
fn the_host_button_reports_only_the_frame_it_was_clicked_in() {
    let mut harness = harness(panel(Some(HostButton {
        label: LABEL.to_owned(),
        tooltip: "Rerun development builds only".into(),
    })));
    assert!(!harness.state().host_button_clicked());

    // The badge colors say, without a tooltip, that this is not a button every user has.
    harness.snapshot("host_button");

    harness.get_by_label_contains(LABEL).click();
    harness.run_steps(1);
    assert!(harness.state().host_button_clicked());

    // Otherwise the host would act on the same click again on the next frame.
    harness.run_steps(1);
    assert!(!harness.state().host_button_clicked());
}

#[test]
fn there_is_no_host_button_unless_the_host_asks_for_one() {
    let harness = harness(panel(None));
    // TODO(emilk/egui#8606): revert to `query_by_*` once invisible widgets no longer end up in the accesskit tree.
    assert!(harness.query_all_by_label_contains(LABEL).count() == 0);
    assert!(!harness.state().host_button_clicked());
}
