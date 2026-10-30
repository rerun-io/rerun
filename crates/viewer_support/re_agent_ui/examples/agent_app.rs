//! Standalone chat app for iterating on `re_agent_ui` without the full viewer.
//!
//! ```sh
//! cargo run -p re_agent_ui --example agent_app -- --mcp "rerun viewer-mcp"
//! ```
//!
//! To skip the agent harness and call a model API directly with your own key, pick the
//! `anthropic-api` or `openai-api` profile. The key is read from `ANTHROPIC_API_KEY` or
//! `OPENAI_API_KEY`:
//!
//! ```sh
//! ANTHROPIC_API_KEY=… cargo run -p re_agent_ui --example agent_app -- --agent anthropic-api
//! OPENAI_API_KEY=… cargo run -p re_agent_ui --example agent_app -- --agent openai-api
//! ```
//!
//! For unattended runs there is a headless mode that renders the same UI through `egui_kittest`.
//! Drive it with `egui-mcp` (see `AGENTS.md`):
//!
//! ```sh
//! EGUI_INSPECTION=1 cargo run -p re_agent_ui --example agent_app -- --headless
//! ```

use std::time::{Duration, Instant};

use clap::Parser;
use re_agent::{AgentProfile, AgentProfileId, AgentSettings, McpServerConfig, SessionContext};
use re_agent_ui::AgentPanel;
use re_ui::UiExt as _;

const SETTINGS_KEY: &str = "agent_settings";

/// Sent to the agent with its first prompt in every session.
const PREAMBLE: &str = "\
You are a coding agent running inside a chat panel of the Rerun agent demo app. \
If an MCP server named `rerun` is available, use it to look at and drive the running Rerun Viewer \
instead of guessing what the user sees. Keep answers short; the panel is narrow.";
const WINDOW_SIZE: [f32; 2] = [700.0, 900.0];

/// Command line flags. Anything that is not set here comes from the persisted settings.
#[derive(Parser)]
#[command(about = "Chat with a coding agent from an egui window")]
struct Args {
    /// Which agent to use: a profile id (claude, codex, gemini, copilot, opencode, goose, cursor),
    /// `anthropic-api` or `openai-api` to call that API directly with the key from the
    /// environment, or a custom command line such as `my-agent --acp`.
    #[arg(long)]
    agent: Option<String>,

    /// Working directory for the agent.
    #[arg(long)]
    cwd: Option<String>,

    /// MCP server command line to hand to the agent, e.g. `rerun viewer-mcp`. Repeatable.
    #[arg(long = "mcp")]
    mcp_servers: Vec<String>,

    /// Run without a window, until `--timeout`. Set `EGUI_INSPECTION=1` to drive it with `egui-mcp`.
    #[arg(long)]
    headless: bool,

    /// How long a headless run stays alive, in seconds.
    #[arg(long, default_value_t = 600)]
    timeout: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    re_log::setup_logging();
    let args = Args::parse();

    // Runs the model-API agents (`--agent anthropic-api`).
    let runtime = tokio::runtime::Runtime::new()?; // NOLINT: the example process owns this runtime
    let async_runtime = re_async::AsyncRuntimeHandle::new_native(runtime.handle().clone());

    if args.headless {
        run_headless(&args, async_runtime);
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("re_agent_ui_example")
            .with_inner_size(WINDOW_SIZE),
        ..Default::default()
    };

    eframe::run_native(
        "Rerun agent",
        native_options,
        Box::new(move |cc| {
            re_ui::apply_style_and_install_loaders(&cc.egui_ctx);

            let stored = cc
                .storage
                .and_then(|storage| eframe::get_value::<AgentSettings>(storage, SETTINGS_KEY));
            let settings = apply_args(stored.unwrap_or_else(default_settings), &args);

            Ok(Box::new(App::new(settings, async_runtime)))
        }),
    )?;

    Ok(())
}

fn run_headless(args: &Args, async_runtime: re_async::AsyncRuntimeHandle) {
    let settings = apply_args(default_settings(), args);
    let app = App::new(settings, async_runtime);

    let mut harness = egui_kittest::Harness::builder()
        .with_size(WINDOW_SIZE)
        .wgpu()
        .build_eframe(|_cc| app);
    re_ui::apply_style_and_install_loaders(&harness.ctx);

    match egui_inspection::attach_from_env(&harness.ctx, Some("agent_app".to_owned())) {
        Ok(true) => re_log::info!("egui inspection enabled"),
        Ok(false) => re_log::warn!("Set EGUI_INSPECTION=1 to drive the headless app"),
        Err(err) => re_log::warn!("Failed to enable egui inspection: {err}"),
    }

    let deadline = Instant::now() + Duration::from_secs(args.timeout);
    while Instant::now() < deadline {
        harness.step();
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Pre-fills an MCP server entry for the Rerun Viewer, using a dev build if one exists.
fn default_settings() -> AgentSettings {
    let dev_rerun =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/rerun");
    let command = if dev_rerun.is_file() {
        format!("{} viewer-mcp", dev_rerun.display())
    } else {
        "rerun viewer-mcp".to_owned()
    };

    AgentSettings {
        mcp_servers: vec![McpServerConfig {
            name: "rerun".to_owned(),
            command_line: command,
            enabled: re_agent::find_executable("rerun").is_some() || dev_rerun.is_file(),
        }],
        ..Default::default()
    }
}

fn apply_args(mut settings: AgentSettings, args: &Args) -> AgentSettings {
    let Args {
        agent,
        cwd,
        mcp_servers,
        headless: _,
        timeout: _,
    } = args;

    if let Some(agent) = agent {
        let profile_id = AgentProfileId::from(agent.clone());
        let is_profile = profile_id == AgentProfileId::CUSTOM
            || profile_id.model_api_provider().is_some()
            || AgentProfile::builtin()
                .iter()
                .any(|profile| profile.id == profile_id);
        if is_profile {
            settings.profile_id = profile_id;
        } else {
            settings.profile_id = AgentProfileId::CUSTOM;
            settings.custom_command_line = agent.clone();
        }
        // Naming the agent on the command line means: use it, do not ask.
        settings.setup_done = true;
    }
    if let Some(cwd) = cwd {
        settings.cwd = cwd.clone();
    }
    for command_line in mcp_servers {
        let command = command_line.split_whitespace().next().unwrap_or("mcp");
        let name = std::path::Path::new(command).file_stem().map_or_else(
            || command.to_owned(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        settings.mcp_servers.retain(|server| server.name != name);
        settings.mcp_servers.push(McpServerConfig {
            name,
            command_line: command_line.clone(),
            enabled: true,
        });
    }
    settings
}

/// The eframe app: just the agent panel.
struct App {
    panel: AgentPanel,
}

impl App {
    fn new(settings: AgentSettings, async_runtime: re_async::AsyncRuntimeHandle) -> Self {
        let mut panel = AgentPanel::new(settings, async_runtime);
        panel.set_context(SessionContext {
            agent_harness_preamble: Some(PREAMBLE.to_owned()),
            model_api_preamble: Some(PREAMBLE.to_owned()),
            additional_directories: Vec::new(),
            default_cwd: None,
            off_limits_directories: Vec::new(),
        });
        Self { panel }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ui.tokens().panel_bg_color))
            .show(ui, |ui| {
                self.panel.ui(ui);
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        Self::ui(self, ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, self.panel.settings());
    }
}
