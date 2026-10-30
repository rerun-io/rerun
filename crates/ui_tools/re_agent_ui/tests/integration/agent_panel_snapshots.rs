//! Snapshot tests for the agent panel, fed with a scripted session instead of a real agent.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_agent::LineDirection;
use re_agent::acp::schema::v1::{
    AuthMethod, AuthMethodTerminal, ConfigOptionUpdate, ContentBlock, ContentChunk, Diff,
    PermissionOption, PermissionOptionKind, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus,
    RequestPermissionRequest, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionId, SessionMode, SessionModeState, SessionUpdate,
    TextContent, ToolCall, ToolCallContent, ToolCallLocation, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, UsageUpdate,
};
use re_agent::{
    AgentEntry, AgentEvent, AgentProfile, AgentProfileId, AgentSession, AgentSettings,
    McpServerConfig, ModelApiProvider, Prompt, PromptImage,
};
use re_agent_ui::{AgentPanel, RECOMMENDED_WIDTH};

/// As wide as a host is meant to make the panel, so the snapshots show what users see.
const SIZE: Vec2 = Vec2::new(RECOMMENDED_WIDTH, 800.0);

fn harness(panel: AgentPanel) -> egui_kittest::Harness<'static, AgentPanel> {
    re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, SIZE).build_ui_state(
        |ui, panel: &mut AgentPanel| {
            panel.ui(ui);
        },
        panel,
    )
}

fn snapshot(name: &str, panel: AgentPanel) {
    let mut harness = harness(panel);
    // `run` would never settle: the chat shows spinners while the agent is busy.
    harness.run_steps(4);
    harness.snapshot(name);
}

fn settings() -> AgentSettings {
    AgentSettings {
        profile_id: AgentProfileId::new("claude"),
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
    let mut panel = AgentPanel::with_agents(settings(), None, agents, Vec::new());
    panel.set_example_prompts(crate::example_prompts());
    panel
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
        config_options: Vec::new(),
    });
}

/// Clicking into the custom command selects "Custom command", so what is typed there is used.
#[test]
fn clicking_the_custom_command_selects_it() {
    let mut harness = harness(panel());
    harness.run_steps(2);
    assert_eq!(
        harness.state().settings().profile_id,
        AgentProfileId::new("claude")
    );

    harness
        .get_by_label("Other agents and custom command")
        .click();
    harness.run_steps(4);
    harness.get_by_role(egui::Role::TextInput).click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().settings().profile_id,
        AgentProfileId::CUSTOM
    );
}

/// A first run on a machine with none of the agents installed, and keys for `model_apis`.
fn first_run_without_agents(available_model_apis: Vec<ModelApiProvider>) -> AgentPanel {
    let agents = AgentProfile::builtin()
        .into_iter()
        .map(|profile| AgentEntry {
            profile,
            executable: None,
        })
        .collect();
    let settings = AgentSettings {
        profile_id: AgentProfileId::default(),
        ..settings()
    };
    AgentPanel::with_agents(settings, None, agents, available_model_apis)
}

/// With no agent and no key, a model API is preselected, and the screen says how to get a key.
#[test]
fn setup_screen_without_agents_or_keys() {
    let mut harness = harness(first_run_without_agents(Vec::new()));
    harness.run_steps(4);
    assert_eq!(
        harness.state().settings().profile_id,
        ModelApiProvider::Anthropic.profile_id()
    );
    assert!(
        harness
            .get_by_label_contains("Start ")
            .accesskit_node()
            .is_disabled()
    );
    harness.snapshot("setup_screen_without_agents_or_keys");
}

/// With no agent but a key, the model API with the key is preselected, ready to start.
#[test]
fn setup_screen_without_agents_with_a_key() {
    let mut harness = harness(first_run_without_agents(vec![ModelApiProvider::OpenAi]));
    harness.run_steps(4);
    assert_eq!(
        harness.state().settings().profile_id,
        ModelApiProvider::OpenAi.profile_id()
    );
    assert!(
        !harness
            .get_by_label_contains("Start ")
            .accesskit_node()
            .is_disabled()
    );
}

/// Picking the model-API card selects a provider even without a key, so the screen can say
/// which variable to set.
#[test]
fn picking_the_model_api_card_without_a_key() {
    let mut harness = harness(panel());
    harness.run_steps(2);

    harness
        .get_by_label("A model API with your own key")
        .click();
    harness.run_steps(4);
    assert_eq!(
        harness.state().settings().profile_id,
        ModelApiProvider::Anthropic.profile_id()
    );
    harness.snapshot("setup_screen_model_api");
}

#[test]
fn setup_screen() {
    snapshot("setup_screen", panel());
}

/// Clicking "Advanced" opens it without leaving a focus outline behind.
#[test]
fn setup_screen_advanced() {
    let mut harness = harness(panel());
    harness.run_steps(2);
    harness.get_by_label("Advanced").click();
    harness.run_steps(4);
    harness.snapshot("setup_screen_advanced");

    harness.get_by_label("MCP servers").scroll_to_me();
    harness.remove_cursor();
    harness.run_steps(4);
    harness.snapshot("setup_screen_advanced_scrolled");
}

/// A phrase that only appears once the particulars are on screen.
const DETAILS_FRAGMENT: &str = "processed on Rerun's behalf by PostHog";

fn snapshot_hovering(name: &str, label: &str) {
    let mut harness = harness(panel());
    harness.run_steps(4);
    harness.get_by_label_contains(label).hover();
    harness.try_run_realtime().ok();
    harness.snapshot(name);
}

/// The particulars are only ever a hover away, so snapshot them where the reader meets them.
#[test]
fn prompt_sharing_details_from_the_checkbox() {
    snapshot_hovering(
        "prompt_sharing_details_checkbox",
        "Share anonymized prompts with Rerun",
    );
}

#[test]
fn prompt_sharing_details_from_the_link() {
    snapshot_hovering("prompt_sharing_details_link", "Details");
}

/// Clicking "Details" opens the particulars, for readers without a steady hover.
#[test]
fn analytics_details_open_on_click() {
    let mut harness = harness(panel());
    harness.run_steps(4);
    assert_eq!(
        harness
            .query_all_by_label_contains(DETAILS_FRAGMENT)
            .count(),
        0,
        "the details should start closed"
    );

    harness.get_by_label("Details").click();
    harness.run_steps(2);
    assert_eq!(
        harness
            .query_all_by_label_contains(DETAILS_FRAGMENT)
            .count(),
        1,
        "clicking should open the details"
    );
}

/// Sharing starts off, the checkbox turns it on, and both states say what they mean.
///
/// The unticked state is in the `setup_screen` snapshot, which is where the panel starts.
#[test]
fn redacted_prompt_sharing_is_opt_in() {
    let mut harness = harness(panel());
    harness.run_steps(4);

    // The checkbox must be reachable without expanding "Advanced":
    assert!(!harness.state().settings().share_redacted_prompts);
    harness
        .get_by_label_contains("Share anonymized prompts with Rerun")
        .click();
    harness.run_steps(4);

    assert!(harness.state().settings().share_redacted_prompts);
    harness.snapshot("setup_screen_prompt_sharing_on");
}

#[test]
fn chat_conversation() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
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
        .push_user("The left camera doesn't show up in the viewer. Fix it.".into());
    for update in updates {
        session.handle_event(AgentEvent::Update(update));
    }

    let mut harness = harness(panel);
    harness.run_steps(4);
    harness.snapshot("chat_conversation");

    // The plan is one line until clicked.
    harness.get_by_label("Plan (1/3)").click();
    harness.run_steps(4);
    harness.snapshot("chat_plan_expanded");

    // The folded tool calls, with the failed one in red.
    harness.get_by_label("Used 4 tools").click();
    // Off the transcript, so no hover times (which depend on the clock) show up.
    harness.hover_at(egui::pos2(1.0, SIZE.y - 1.0));
    harness.run_steps(4);
    harness.snapshot("chat_tool_calls_expanded");
}

/// Once a turn has ended, its tool calls fold into one row.
#[test]
fn chat_finished_turn() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    session
        .transcript_mut()
        .push_user("Help me detect missing frames".into());

    let updates = [
        SessionUpdate::AgentMessageChunk(ContentChunk::new(text(
            "I'll start by looking at what's in this recording.",
        ))),
        SessionUpdate::ToolCall(
            ToolCall::new("schema-1", "Read recording schema").status(ToolCallStatus::Completed),
        ),
        SessionUpdate::ToolCall(
            ToolCall::new("blueprint-1", "Check blueprint").status(ToolCallStatus::Completed),
        ),
        SessionUpdate::ToolCall(
            ToolCall::new("run-1", "Run detect_gaps.py")
                .kind(ToolKind::Execute)
                .status(ToolCallStatus::Completed),
        ),
        SessionUpdate::AgentMessageChunk(ContentChunk::new(text(
            "Found 3 gaps in `/camera/wrist`. The other cameras are complete.",
        ))),
    ];
    for update in updates {
        session.handle_event(AgentEvent::Update(update));
    }

    snapshot("chat_finished_turn", panel);
}

/// Tool calls fold once the turn has ended, failed or not, and expand again on click.
#[test]
fn tool_calls_fold_when_turn_ended() {
    for (second_status, turn_running, folded) in [
        (ToolCallStatus::Completed, false, true),
        (ToolCallStatus::Failed, false, true),
        (ToolCallStatus::Completed, true, false),
    ] {
        let mut panel = panel();
        panel.show_chat();
        let session = panel.session_mut();
        open_session(session);
        session
            .transcript_mut()
            .push_user("Check the recording".into());
        if turn_running {
            session.begin_test_turn();
        }
        for call in [
            ToolCall::new("schema-1", "Read recording schema").status(ToolCallStatus::Completed),
            ToolCall::new("blueprint-1", "Check blueprint").status(second_status),
        ] {
            session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(call)));
        }

        let mut harness = harness(panel);
        harness.run_steps(4);
        assert_eq!(
            harness.query_by_label("Check blueprint").is_none(),
            folded,
            "{second_status:?}, turn running: {turn_running}",
        );

        if folded {
            harness.get_by_label("Used 2 tools").click();
            harness.run_steps(2);
            harness.get_by_label("Check blueprint");
        }
    }
}

/// Clicking the title of a tool call expands it, not just the triangle.
#[test]
fn tool_call_expands_on_title_click() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    session
        .transcript_mut()
        .push_user("Check the recording".into());
    session.begin_test_turn();
    let call = ToolCall::new("schema-1", "Read recording schema")
        .status(ToolCallStatus::Completed)
        .raw_input(serde_json::json!({ "entity": "/world" }));
    session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(call)));

    let mut harness = harness(panel);
    harness.run_steps(4);
    assert!(
        harness.query_by_label("Input").is_none(),
        "a completed tool call starts collapsed"
    );

    harness.get_by_label("Read recording schema").click();
    harness.run_steps(2);
    harness.get_by_label("Input");
}

/// Hovering an entry shows when it was created.
#[test]
fn chat_hover_timestamp() {
    crate::use_utc();

    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    session.transcript_mut().push_user("open a dataset".into());
    session.handle_event(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
        ContentChunk::new(text("Check viewer for servers and datasets first.\n")),
    )));
    session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
        ToolCall::new("search-1", "ToolSearch")
            .kind(ToolKind::Search)
            .status(ToolCallStatus::Completed),
    )));
    session.transcript_mut().push_user(
        "The left camera doesn't show up in the viewer. Fix it. \
         And here is a much longer sentence so that the prompt wraps over several lines."
            .into(),
    );
    crate::pin_entry_times(session);

    let mut harness = harness(panel);
    harness.run_steps(4);

    harness.get_by_label_contains("ToolSearch").hover();
    harness.try_run_realtime().ok();
    harness.snapshot("chat_hover_timestamp_one_line");

    // The header shows the same text, so pick the message below it.
    harness
        .get_all_by_label("open a dataset")
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the user message")
        .hover();
    harness.try_run_realtime().ok();
    harness.snapshot("chat_hover_timestamp_user");

    harness
        .get_by_label_contains("much longer sentence")
        .hover();
    harness.try_run_realtime().ok();
    harness.snapshot("chat_hover_timestamp_multi_line");
}

/// A user message taller than the panel still shows its timestamp when its top is scrolled away.
#[test]
fn chat_hover_timestamp_user_scrolled() {
    crate::use_utc();

    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    let lines = (1..=60).map(|i| format!("Line {i}")).collect::<Vec<_>>();
    session.transcript_mut().push_user(lines.join("\n").into());
    crate::pin_entry_times(session);

    let mut harness = harness(panel);
    harness.run_steps(4);

    let bubble = harness.get_by_label_contains("Line 60").rect();
    harness.hover_at(egui::pos2(bubble.center().x, bubble.bottom() - 20.0));
    harness.try_run_realtime().ok();
    harness.snapshot("chat_hover_timestamp_user_scrolled");
}

/// A small solid image, standing in for a pasted screenshot.
fn pasted_image() -> PromptImage {
    let resolution = [64, 48];
    let pixels = image::RgbImage::from_pixel(64, 48, image::Rgb([90, 120, 160]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(pixels)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("a solid PNG always encodes");
    PromptImage {
        bytes: bytes.into(),
        mime_type: "image/png".to_owned(),
        resolution,
    }
}

/// A pasted image stays in the history, so the conversation still reads as a whole later on.
#[test]
fn chat_pasted_image() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);

    session.transcript_mut().push_user(Prompt {
        text: "What is wrong with this plot?".to_owned(),
        images: vec![pasted_image()],
    });

    snapshot("chat_pasted_image", panel);
}

/// An MCP server that failed to start is an error in the transcript, with what Rerun asked for.
#[test]
fn chat_mcp_startup_failure() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    session
        .transcript_mut()
        .push_mcp_startup_failure(re_agent::McpStartupFailure {
            server_name: "rerun".to_owned(),
            error: "missing field `cacheScope`".to_owned(),
            requested_command: Some(PathBuf::from("rerun")),
            requested_args: vec![
                "viewer-mcp".to_owned(),
                "--endpoint".to_owned(),
                "rerun+http://127.0.0.1:9876/proxy".to_owned(),
            ],
            hint: Some("Update the agent's ACP adapter, then restart the agent.".to_owned()),
        });

    snapshot("chat_mcp_startup_failure", panel);
}

#[test]
fn chat_permission_request() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);

    session
        .transcript_mut()
        .push_user("Create a file /tmp/probe.txt containing the word 'probe'.".into());
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
    let session = panel.session_mut();
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
    let session = panel.session_mut();
    open_session(session);

    session
        .transcript_mut()
        .push_user("List the recordings.".into());
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
    let session = panel.session_mut();
    open_session(session);

    session
        .transcript_mut()
        .push_user("Make the left camera show up in the viewer.".into());
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
fn chat_empty() {
    let mut panel = panel();
    panel.show_chat();
    open_session(panel.session_mut());

    snapshot("chat_empty", panel);
}

#[test]
fn chat_login_required() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
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

/// The conversation list, with one conversation in each status.
#[test]
fn conversation_list() {
    enum State {
        NeedsInput,
        Errored,
        Working,
        NewMessage,
        NothingNew,
    }

    let mut panel = panel();
    for (prompt, state) in [
        ("Plot the IMU", State::NothingNew),
        ("Make the rectangle green", State::NewMessage),
        ("Export the wrist camera", State::Working),
        ("Load the dataset", State::Errored),
        ("Find failed grasps", State::NeedsInput),
    ] {
        panel.new_conversation();
        panel.show_chat();
        let session = panel.session_mut();
        open_session(session);
        session.transcript_mut().push_user(prompt.into());
        match state {
            State::NeedsInput => {
                session.begin_test_turn();
                session.push_test_permission_request(RequestPermissionRequest::new(
                    SessionId::new("test-session"),
                    ToolCallUpdate::new("exec-1", ToolCallUpdateFields::new()),
                    vec![PermissionOption::new(
                        "allow",
                        "Yes",
                        PermissionOptionKind::AllowOnce,
                    )],
                ));
            }
            State::Errored => {
                session
                    .transcript_mut()
                    .push_note("The agent stopped: rate limit reached", true);
            }
            State::Working => session.begin_test_turn(),
            State::NewMessage | State::NothingNew => {}
        }
        panel.poll_events();
        if matches!(state, State::NewMessage) {
            // Arrives after the user left, so it is unseen.
            panel
                .session_mut()
                .handle_event(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
                    ContentChunk::new(text("Done!")),
                )));
        }
    }
    panel.new_conversation();
    panel.show_chat();
    open_session(panel.session_mut());

    let mut harness = harness(panel);
    harness.run_steps(4);
    harness.get_by_label("Back to conversations").click();
    harness.run_steps(4);
    // Hovering shows the close button next to the status.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Make the rectangle green")
        .hover();
    harness.run_steps(4);
    harness.snapshot("conversation_list");
}

#[test]
fn chat_queued_prompts() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);

    session
        .transcript_mut()
        .push_user("Make the left camera show up in the viewer.".into());
    session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
        ToolCall::new("read-1", "Read log_cameras.py")
            .kind(ToolKind::Read)
            .status(ToolCallStatus::InProgress),
    )));
    session.begin_test_turn();
    assert!(session.send_prompt("Then also fix the tests.".into()));
    assert!(session.send_prompt("And update the README:\n- mention the new entity path".into()));

    snapshot("chat_queued_prompts", panel);
}

/// Right after the user sends a prompt, before the agent's first reply, the transcript shows that
/// the agent is working.
#[test]
fn chat_waiting_for_the_first_reply() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);

    session
        .transcript_mut()
        .push_user("Make the left camera show up in the viewer.".into());
    session.begin_test_turn();

    snapshot("chat_waiting_for_the_first_reply", panel);
}

/// The footer names the concrete model behind the agent's default, and opens a list of the others.
#[test]
fn chat_model_picker() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    let models = vec![
        SessionConfigSelectOption::new("default", "Default (recommended)")
            .description("Opus 5.5".to_owned()),
        SessionConfigSelectOption::new("opus", "Opus 5.5")
            .description("For complex work and everyday tasks".to_owned()),
        SessionConfigSelectOption::new("sonnet", "Sonnet 5.5")
            .description("Most efficient for simpler tasks".to_owned()),
        SessionConfigSelectOption::new("haiku", "Haiku 4.5")
            .description("Fastest for quick answers".to_owned()),
    ];
    session.handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
        ConfigOptionUpdate::new(vec![
            SessionConfigOption::select("model", "Model", "default", models)
                .category(SessionConfigOptionCategory::Model),
        ]),
    )));

    let mut harness = harness(panel);
    harness.run_steps(4);
    harness.get_by_value("Opus 5.5").click();
    harness.run_steps(4);
    harness.snapshot("chat_model_picker");
}

/// In a narrow panel, the composer footer truncates instead of running under the stop button.
#[test]
fn chat_narrow_composer() {
    let mut panel = panel();
    panel.show_chat();
    let session = panel.session_mut();
    open_session(session);
    session.handle_event(AgentEvent::Update(SessionUpdate::ConfigOptionUpdate(
        ConfigOptionUpdate::new(vec![
            SessionConfigOption::select(
                "model",
                "Model",
                "default",
                vec![SessionConfigSelectOption::new(
                    "default",
                    "Default (recommended)",
                )],
            )
            .category(SessionConfigOptionCategory::Model),
        ]),
    )));
    session.handle_event(AgentEvent::Update(SessionUpdate::UsageUpdate(
        UsageUpdate::new(21_300, 1_000_000),
    )));
    session.handle_event(AgentEvent::Log {
        direction: LineDirection::Stderr,
        line: "warning: falling back to the bundled Node.js".to_owned(),
    });
    session.begin_test_turn();

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(260.0, 400.0))
            .build_ui_state(
                |ui, panel: &mut AgentPanel| {
                    panel.ui(ui);
                },
                panel,
            );
    harness.run_steps(4);
    harness.snapshot("chat_narrow_composer");
}
