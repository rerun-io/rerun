//! Snapshot tests for the agent panel, fed with a scripted session instead of a real agent.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_agent_ui::acp::LineDirection;
use re_agent_ui::acp::schema::v1::{
    AuthMethod, AuthMethodTerminal, ContentBlock, ContentChunk, Diff, PermissionOption,
    PermissionOptionKind, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus,
    RequestPermissionRequest, SessionId, SessionMode, SessionModeState, SessionUpdate, TextContent,
    ToolCall, ToolCallContent, ToolCallLocation, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, UsageUpdate,
};
use re_agent_ui::{
    AgentEntry, AgentEvent, AgentPanel, AgentProfile, AgentSession, AgentSettings, McpServerConfig,
};

const SIZE: Vec2 = Vec2::new(600.0, 800.0);

fn snapshot(name: &str, panel: AgentPanel) {
    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel,
        );
    // `run` would never settle: the chat shows spinners while the agent is busy.
    harness.run_steps(4);
    harness.snapshot(name);
}

fn settings() -> AgentSettings {
    AgentSettings {
        profile_id: "claude".to_owned(),
        cwd: "/home/user/robot-project".to_owned(),
        mcp_servers: vec![McpServerConfig {
            name: "rerun".to_owned(),
            command_line: "rerun viewer-mcp".to_owned(),
            enabled: true,
        }],
        ..Default::default()
    }
}

/// A fixed set of agents, so the snapshot does not depend on what is installed on the machine.
fn panel() -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| {
            let installed = matches!(profile.id.as_str(), "claude" | "codex");
            AgentEntry {
                executable: installed
                    .then(|| PathBuf::from("/usr/local/bin").join(&profile.command)),
                profile,
            }
        })
        .collect();
    AgentPanel::with_agents(settings(), agents)
}

fn text(text: &str) -> ContentBlock {
    ContentBlock::Text(TextContent::new(text))
}

/// Opens a session so the chat view is shown, with the mode picker populated.
fn open_session(session: &mut AgentSession) {
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
                SessionMode::new("bypassPermissions", "Bypass permissions"),
            ],
        )),
    });
}

#[test]
fn setup_screen() {
    snapshot("setup_screen", panel());
}

#[test]
fn chat_conversation() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    open_session(session);

    let updates = [
        SessionUpdate::AgentThoughtChunk(ContentChunk::new(text(
            "The user wants the camera to show up. I should look at the logging code first.",
        ))),
        SessionUpdate::ToolCall(
            ToolCall::new("read-1", "Read log_cameras.py")
                .kind(ToolKind::Read)
                .status(ToolCallStatus::Completed)
                .locations(vec![
                    ToolCallLocation::new(PathBuf::from("src/log_cameras.py")).line(42),
                ]),
        ),
        SessionUpdate::ToolCall(
            ToolCall::new("edit-1", "Fix the entity path")
                .kind(ToolKind::Edit)
                .status(ToolCallStatus::Completed)
                .content(vec![ToolCallContent::from(
                    Diff::new(
                        PathBuf::from("src/log_cameras.py"),
                        "rr.log(\"world/camera/left\", rr.Image(left))\n",
                    )
                    .old_text("rr.log(\"world/camera_left\", rr.Image(left))\n"),
                )]),
        ),
        SessionUpdate::ToolCall(
            ToolCall::new("mcp-1", "mcp__rerun__viewer_state")
                .kind(ToolKind::Other)
                .status(ToolCallStatus::Failed)
                .raw_input(serde_json::json!({}))
                .raw_output(serde_json::json!(
                    "transport error: no viewer listening on http://127.0.0.1:9876"
                )),
        ),
        SessionUpdate::ToolCall(
            ToolCall::new("search-1", "Grep for camera_left")
                .kind(ToolKind::Search)
                .status(ToolCallStatus::InProgress),
        ),
        SessionUpdate::AgentMessageChunk(ContentChunk::new(text(
            "Found it. The left camera was logged under `world/camera_left`, but the blueprint expects `world/camera/left`.\n\n",
        ))),
        SessionUpdate::AgentMessageChunk(ContentChunk::new(text(
            "I changed the entity path:\n\n```python\nrr.log(\"world/camera/left\", rr.Image(left))\n```\n\nNext steps:\n\n1. Re-run the script\n2. Check the viewer\n\nSee https://rerun.io/docs/concepts/entity-path for the entity path rules.\n",
        ))),
        SessionUpdate::Plan(Plan::new(vec![
            PlanEntry::new(
                "Find where the camera is logged",
                PlanEntryPriority::High,
                PlanEntryStatus::Completed,
            ),
            PlanEntry::new(
                "Fix the entity path",
                PlanEntryPriority::High,
                PlanEntryStatus::InProgress,
            ),
            PlanEntry::new(
                "Verify in the viewer",
                PlanEntryPriority::Medium,
                PlanEntryStatus::Pending,
            ),
        ])),
        SessionUpdate::UsageUpdate(UsageUpdate::new(21_300, 1_000_000)),
    ];

    // The user's own prompt is added by the UI, not by the agent:
    session
        .transcript_mut()
        .push_user("The left camera doesn't show up in the viewer. Fix it.".to_owned());
    for update in updates {
        session.handle_event(AgentEvent::Update(update));
    }

    snapshot("chat_conversation", panel);
}

#[test]
fn chat_permission_request() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    open_session(session);

    session
        .transcript_mut()
        .push_user("Create a file /tmp/probe.txt containing the word 'probe'.".to_owned());
    session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
        ToolCall::new("exec-1", "Write probe file")
            .kind(ToolKind::Execute)
            .status(ToolCallStatus::Pending)
            .raw_input(serde_json::json!({
                "command": "echo probe > /tmp/probe.txt",
                "description": "Write probe file",
            })),
    )));
    session.push_test_permission_request(RequestPermissionRequest::new(
        SessionId::new("test-session"),
        ToolCallUpdate::new("exec-1", ToolCallUpdateFields::new()),
        vec![
            PermissionOption::new("allow", "Yes", PermissionOptionKind::AllowOnce),
            PermissionOption::new(
                "allow-always",
                "Yes, and allow access to tmp/",
                PermissionOptionKind::AllowAlways,
            ),
            PermissionOption::new("reject", "No", PermissionOptionKind::RejectOnce),
        ],
    ));

    snapshot("chat_permission_request", panel);
}

/// The agent log with its direction markers and a stderr line, expanded via the footer toggle.
///
/// Also guards the non-ASCII glyphs the chat relies on (arrows here, `⏎`, `⇧`, `…`, `·` in the
/// other snapshots): a font change that drops one of them shows up as a tofu box.
#[test]
fn chat_agent_log() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    open_session(session);

    for (direction, line) in [
        (
            LineDirection::Stdin,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}"#,
        ),
        (
            LineDirection::Stdout,
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentInfo":{"name":"claude-agent-acp"}}}"#,
        ),
        (
            LineDirection::Stderr,
            "warning: falling back to the bundled Node.js",
        ),
    ] {
        session.handle_event(AgentEvent::Log {
            direction,
            line: line.to_owned(),
        });
    }

    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(4);
    harness.get_by_label_contains("Agent log (").click();
    harness.run_steps(4);
    harness.snapshot("chat_agent_log");
}

/// A table wider than the panel must scroll on its own, not widen the messages after it.
#[test]
fn chat_wide_table() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    open_session(session);

    session
        .transcript_mut()
        .push_user("List the recordings.".to_owned());
    let table = "| Recording | Application | Timelines | Duration | Size on disk | Notes |\n\
                 |-----------|-------------|-----------|----------|--------------|-------|\n\
                 | `episode_000001` | `warehouse_pick` | `frame`, `time` | 2m 13s | 1.2 GB | left camera missing after frame 800 |\n\
                 | `episode_000002` | `warehouse_pick` | `frame`, `time` | 1m 58s | 1.1 GB | ok |\n";
    session.handle_event(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
        ContentChunk::new(text(&format!(
            "Two recordings are open:\n\n{table}\nThe first one is the interesting one: the left camera \
             stops being logged after frame 800, which is why the view goes blank there."
        ))),
    )));

    snapshot("chat_wide_table", panel);
}

/// Claude Code's plan mode asks for approval with the whole plan in the tool input.
#[test]
fn chat_plan_approval() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    open_session(session);

    session
        .transcript_mut()
        .push_user("Make the left camera show up in the viewer.".to_owned());
    session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
        ToolCall::new("plan-1", "Exit plan mode")
            .kind(ToolKind::SwitchMode)
            .status(ToolCallStatus::Pending)
            .raw_input(serde_json::json!({
                "plan": "# Fix the left camera\n\n## Steps\n\n1. Rename the entity to `world/camera/left`.\n2. Re-run `log_cameras.py`.\n\n| File | Change |\n|------|--------|\n| `log_cameras.py` | entity path |\n",
                "planFilePath": "~/.claude/plans/fix-left-camera.md",
                "allowedPrompts": [{"tool": "Bash", "prompt": "run the script"}],
            })),
    )));
    session.push_test_permission_request(RequestPermissionRequest::new(
        SessionId::new("test-session"),
        ToolCallUpdate::new("plan-1", ToolCallUpdateFields::new()),
        vec![
            PermissionOption::new("allow", "Yes, start", PermissionOptionKind::AllowOnce),
            PermissionOption::new(
                "reject",
                "No, keep planning",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    ));

    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE)
        .build_ui_state(
            |ui, panel: &mut AgentPanel| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                panel.ui(ui);
            },
            panel,
        );
    harness.run_steps(4);
    harness.snapshot("chat_plan_approval");

    harness.get_by_label_contains("plan (").click();
    harness.run_steps(4);
    harness.snapshot("chat_plan_approval_expanded");
}

#[test]
fn chat_login_required() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    session.handle_event(AgentEvent::Status("Starting session…".to_owned()));
    session.handle_event(AgentEvent::AuthRequired {
        methods: vec![AuthMethod::Terminal(
            AuthMethodTerminal::new("claude-login", "Log in with Claude Code")
                .description("Opens the browser to sign in to your Anthropic account.")
                .args(vec!["claude".to_owned(), "/login".to_owned()]),
        )],
        message: "Authentication required".to_owned(),
    });

    snapshot("chat_login_required", panel);
}

#[test]
fn chat_queued_prompts() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut().expect("one conversation");
    open_session(session);

    session
        .transcript_mut()
        .push_user("Make the left camera show up in the viewer.".to_owned());
    session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
        ToolCall::new("read-1", "Read log_cameras.py")
            .kind(ToolKind::Read)
            .status(ToolCallStatus::InProgress),
    )));
    session.begin_test_turn();
    assert!(session.send_prompt("Then also fix the tests."));
    assert!(session.send_prompt("And update the README:\n- mention the new entity path"));

    snapshot("chat_queued_prompts", panel);
}
