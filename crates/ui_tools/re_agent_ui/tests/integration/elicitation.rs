use egui::{Role, Vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_agent::acp::schema::v1::CreateElicitationRequest;
use re_agent::{AgentSession, TranscriptItem};
use re_agent_ui::AgentPanel;
use serde_json::json;

use super::chat_input::ready_panel;

fn question(message: &str, schema: &serde_json::Value) -> CreateElicitationRequest {
    serde_json::from_value(json!({
        "mode": "form",
        "sessionId": "test-session",
        "message": message,
        "requestedSchema": schema,
    }))
    .expect("valid question")
}

fn harness(request: CreateElicitationRequest) -> egui_kittest::Harness<'static, AgentPanel> {
    let mut panel = ready_panel();
    let session = panel.session_mut();
    session.begin_test_turn();
    session.push_test_elicitation_request(request);
    re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(re_agent_ui::RECOMMENDED_WIDTH, 800.0),
    )
    .build_ui_state(
        |ui, panel: &mut AgentPanel| {
            panel.ui(ui);
        },
        panel,
    )
}

fn last_answer_note(session: &AgentSession) -> &str {
    let Some(TranscriptItem::Note {
        text,
        is_error: false,
    }) = session.transcript().items.last().map(|entry| &entry.item)
    else {
        panic!(
            "Expected an answer note, got: {}",
            session.transcript().to_plain_text()
        );
    };
    text
}

#[test]
fn python_environment_choice_and_follow_up() {
    super::use_utc();
    let mut harness = harness(question(
        "Which Python environment should I use?",
        &json!({
            "type": "object",
            "properties": {
                "environment": { "type": "string", "title": "Python environment", "oneOf": [
                    {"const": "existing", "title": "Use an existing Python environment", "description": "Reuse your installed Rerun SDK."},
                    {"const": "temporary", "title": "Use a fresh, temporary environment"}
                ]},
                "other": {"type": "string", "title": "Other"}
            },
            "required": ["environment"]
        }),
    ));
    harness.run_steps(4);
    assert!(harness.get_by_label("Send").accesskit_node().is_disabled());
    harness
        .get_by_label("Use an existing Python environment")
        .click();
    harness.run_steps(2);
    harness.snapshot("python_environment_question");
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert!(session.pending_elicitations().is_empty());
    assert_eq!(
        last_answer_note(session),
        "Which Python environment should I use?\nPython environment: Use an existing Python environment"
    );
    session.push_test_elicitation_request(question("What is the interpreter path or project run command?", &json!({
        "type": "object",
        "properties": {"interpreter": {"type": "string", "title": "Interpreter path or run command", "minLength": 1}},
        "required": ["interpreter"]
    })));
    harness.run_steps(3);
    assert!(harness.get_by_label("Send").accesskit_node().is_disabled());
    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("/home/user/project/.venv/bin/python");
    harness.run_steps(2);
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert!(session.pending_elicitations().is_empty());
    assert_eq!(
        last_answer_note(session),
        "What is the interpreter path or project run command?\nInterpreter path or run command: /home/user/project/.venv/bin/python"
    );
}

#[test]
fn multi_select_and_custom_answer() {
    let mut harness = harness(question(
        "Which data should I inspect?",
        &json!({
            "type": "object",
            "properties": {
                "streams": {"type": "array", "title": "Streams", "items": {"anyOf": [
                    {"const": "camera", "title": "Camera", "description": "Inspect camera images and their associated calibration data."},
                    {"const": "lidar", "title": "Lidar", "description": "Inspect point clouds."}
                ]}, "minItems": 1},
                "custom": {"type": "string", "title": "Other"}
            },
            "required": ["streams"]
        }),
    ));
    harness.run_steps(3);
    harness.get_by_label("Camera").click();
    harness.run_steps(2);
    harness.get_by_label("Lidar").click();
    harness.run_steps(2);
    harness.snapshot("multi_select_question");
    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("Also inspect the robot pose");
    harness.run_steps(2);
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert!(session.pending_elicitations().is_empty());
    assert_eq!(
        last_answer_note(session),
        "Which data should I inspect?\nOther: Also inspect the robot pose\nStreams: camera, lidar"
    );
}

#[test]
fn skipping_a_question_does_not_cancel_the_turn() {
    let mut harness = harness(question(
        "Where should I save the script?",
        &json!({
            "type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]
        }),
    ));
    harness.run_steps(3);
    harness.get_by_label("Skip").click();
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert!(session.pending_elicitations().is_empty());
    assert!(session.turn_in_progress());
    assert_eq!(
        last_answer_note(session),
        "Where should I save the script?\nSkipped"
    );
}

#[test]
fn custom_answer_without_selecting_an_option() {
    let mut harness = harness(question(
        "Which Python environment should I use?",
        &json!({
            "type": "object",
            "properties": {
                "question_0": {"type": "string", "oneOf": [
                    {"const": "existing", "title": "Existing environment"},
                    {"const": "temporary", "title": "Temporary environment"}
                ]},
                "question_0_custom": {"type": "string", "title": "Other"}
            }
        }),
    ));
    harness.run_steps(3);
    assert!(harness.get_by_label("Send").accesskit_node().is_disabled());
    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("   ");
    harness.run_steps(2);
    assert!(harness.get_by_label("Send").accesskit_node().is_disabled());
    let input = harness.get_by_role(Role::TextInput);
    input.type_text("Run Python inside my Docker container");
    harness.run_steps(2);
    harness.get_by_label("Send").click();
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert!(session.pending_elicitations().is_empty());
    assert_eq!(
        last_answer_note(session),
        "Which Python environment should I use?\nOther:    Run Python inside my Docker container"
    );
}

#[test]
fn enter_in_a_text_field_sends_the_answer() {
    let mut harness = harness(question(
        "What is the URL of latest-eu?",
        &json!({
            "type": "object",
            "properties": {"url": {"type": "string", "title": "URL"}}
        }),
    ));
    harness.run_steps(3);
    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert_eq!(
        session.pending_elicitations().len(),
        1,
        "a blank answer is not sent"
    );

    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("api.latest-eu.cloud.rerun.io");
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    let session = harness.state_mut().session_mut();
    assert!(session.pending_elicitations().is_empty());
    assert_eq!(
        last_answer_note(session),
        "What is the URL of latest-eu?\nURL: api.latest-eu.cloud.rerun.io"
    );
}

/// The agent waits on the user, so nothing shows progress: not the turn, and not the tool call
/// that asked.
#[test]
fn no_progress_indicator_while_a_question_is_open() {
    use re_agent::AgentEvent;
    use re_agent::acp::schema::v1::{SessionUpdate, ToolCall, ToolCallStatus};

    let mut harness = harness(question(
        "What is the URL of latest-eu?",
        &json!({
            "type": "object",
            "properties": {"url": {"type": "string", "title": "URL"}}
        }),
    ));
    harness
        .state_mut()
        .session_mut()
        .handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
            ToolCall::new("ask-1", "Ask for the URL").status(ToolCallStatus::InProgress),
        )));
    harness.run_steps(3);
    assert!(harness.query_by_role(Role::ProgressIndicator).is_none());

    harness.get_by_label("Skip").click();
    harness.run_steps(2);
    assert!(
        harness
            .query_all_by_role(Role::ProgressIndicator)
            .next()
            .is_some()
    );
}
