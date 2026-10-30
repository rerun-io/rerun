//! The agent panel: a chat with an agent that runs next to the viewer and drives it, through
//! `rerun viewer-mcp` for an agent harness, or through the viewer-control tools for a model-API
//! agent.
//!
//! This crate holds everything that ties the agent to the Rerun Viewer: the preambles, the
//! viewer-control toolbox, the harness setup, the embedded skills and docs, feedback, and
//! analytics. Chat UI that any egui app could use goes in [`re_agent_ui`] instead.

use std::collections::VecDeque;
use std::path::Path;

use re_agent::AgentSettings;
use re_agent_ui::AgentPanel;
use re_async::AsyncRuntimeHandle;
use re_ui::{ContextExt as _, UiExt as _};

mod agent_analytics;
mod feedback;
#[cfg(agent_harness_backend)]
mod harness;
mod preamble;
mod product_feedback;
mod reporting;
#[cfg(agent_harness_backend)]
mod self_improve;
mod viewer_toolbox;

pub use viewer_toolbox::ViewerControlCall;

/// Where the skills are unpacked inside the agent directory.
#[cfg(agent_harness_backend)]
const SKILLS_SUBDIR: &str = "agent_context/skills";

/// Where the documentation and snippets are unpacked inside the agent directory.
#[cfg(agent_harness_backend)]
const DOCS_SUBDIR: &str = "agent_context/docs";

/// Offered in an empty conversation while a recording is open.
const RECORDING_EXAMPLE_PROMPTS: [&str; 3] = [
    "Set up a blueprint for this recording",
    "Help me fix the errors in this recording",
    "Summarize what is in this recording",
];

/// Offered in an empty conversation while no recording is open.
const NO_RECORDING_EXAMPLE_PROMPTS: [&str; 3] = [
    "Help me open a dataset in Rerun",
    "Help me log my own data to Rerun",
    "Show me an example recording",
];

/// The agent chat, configured for the viewer it lives in.
#[derive(Default)]
pub struct ViewerAgentPanel {
    /// Created the first time the panel opens, since initialization probes for installed agents.
    panel: Option<AgentPanel>,

    /// Set when the panel is opened, so the prompt input takes focus once there is one.
    focus_input: bool,

    /// The example prompts last handed to [`Self::panel`].
    example_prompts: &'static [&'static str],

    /// Prompts waiting for the agent panel to be created — for example, prompts from
    /// the command palette submitted before the agent panel has been opened.
    ///
    /// Use `push_back()` to enqueue, and `pop_front()` to get the next pending prompt.
    pending_prompts: VecDeque<String>,

    #[cfg(agent_harness_backend)]
    harness: harness::Harness,

    reporting: reporting::Reporting,

    /// Viewer-control operations that a model-API agent asked for, to be run on the UI thread.
    /// Set up with the panel.
    viewer_control_calls: Option<crossbeam::channel::Receiver<viewer_toolbox::QueuedCall>>,
}

impl ViewerAgentPanel {
    /// Takes the viewer-control operations that the agent asked for since the last call.
    pub fn take_viewer_control_calls(&self) -> Vec<ViewerControlCall> {
        self.viewer_control_calls
            .as_ref()
            .map(|calls| {
                calls
                    .try_iter()
                    .filter_map(viewer_toolbox::QueuedCall::into_call)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Queue feedback that an agent sent about Rerun, to be sent.
    ///
    /// Its text is only sent when `share_text` is set, after the panel's agent has redacted it.
    pub fn submit_product_feedback(
        &mut self,
        request: re_protos::viewer_control::v1alpha1::SubmitFeedbackRequest,
        share_text: bool,
    ) {
        self.reporting
            .submit_product_feedback(self.panel.as_ref(), request, share_text);
    }

    /// Queue a nonblank prompt in the active conversation, buffering it until the panel exists.
    /// Requests input focus; the caller must open the panel.
    pub fn submit_prompt(&mut self, prompt: String) {
        if prompt.trim().is_empty() {
            return;
        }

        self.request_input_focus();
        if let Some(panel) = self.panel.as_mut() {
            panel.queue_prompt(prompt);
        } else {
            self.pending_prompts.push_back(prompt);
        }
    }

    /// Give the prompt input keyboard focus once there is a panel to give it to.
    pub fn request_input_focus(&mut self) {
        self.focus_input = true;
    }

    /// Show the panel and persist changed settings.
    ///
    /// `open` lives in `AppState` so that it survives a restart.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        open: &mut bool,
        settings: &mut AgentSettings,
        async_runtime: &AsyncRuntimeHandle,
        viewer_endpoint: Option<&str>,
        cache_dir: Option<&Path>,
        is_in_rerun_workspace: bool,
        recording: Option<&re_entity_db::EntityDb>,
    ) {
        let was_open = *open;
        if *open && self.panel.is_none() {
            self.initialize(
                ui.ctx(),
                settings.clone(),
                async_runtime,
                viewer_endpoint,
                cache_dir,
                is_in_rerun_workspace,
            );
        }

        let Self {
            panel,
            focus_input,
            example_prompts,
            pending_prompts,
            ..
        } = self;
        if let Some(panel) = panel.as_mut() {
            let prompts: &'static [&'static str] = if recording.is_some() {
                &RECORDING_EXAMPLE_PROMPTS
            } else {
                &NO_RECORDING_EXAMPLE_PROMPTS
            };
            if *example_prompts != prompts {
                panel.set_example_prompts(prompts.iter().copied().map(str::to_owned).collect());
                *example_prompts = prompts;
            }
            for prompt in pending_prompts.drain(..) {
                panel.queue_prompt(prompt);
            }
            // A collapsed panel never runs its `ui`, and its agents would otherwise
            // sit on their messages until the user opens it again.
            panel.poll_events();

            if *focus_input {
                panel.request_input_focus();
                *focus_input = false;
            }
        }
        egui::Panel::right("agent_panel")
            .default_size(re_agent_ui::RECOMMENDED_WIDTH)
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                ..Default::default()
            })
            .show_collapsible(ui, open, |ui| {
                ui.name_panel("Agent panel");

                if let Some(panel) = panel {
                    panel.ui(ui);
                    settings.clone_from(panel.settings());
                }
            });

        // Dragging the collapsed panel open does not go through `toggle_agent_panel`:
        self.focus_input |= !was_open && *open;

        let Some(panel) = &mut self.panel else {
            return;
        };
        #[cfg(agent_harness_backend)]
        self.harness.handle_self_improve_request(panel);
        // Before the redactors run, so that turning sharing off drops what is queued before any
        // of it can be sent.
        let turns = panel.take_finished_turns();
        self.reporting.record_finished_turns(panel, turns);
        self.reporting.ui(panel, ui);
    }

    /// Initialize the inner panel.
    ///
    /// `async_runtime` runs model-API agents, and `egui_ctx` is woken for their tool calls.
    /// `viewer_endpoint`, `cache_dir`, and `is_in_rerun_workspace` only matter to agent
    /// harnesses; see [`harness::Harness::new_panel`].
    fn initialize(
        &mut self,
        egui_ctx: &egui::Context,
        settings: AgentSettings,
        async_runtime: &AsyncRuntimeHandle,
        viewer_endpoint: Option<&str>,
        cache_dir: Option<&Path>,
        is_in_rerun_workspace: bool,
    ) {
        re_tracing::profile_function!();

        let mut panel = cfg_select! {
            agent_harness_backend => {
                self.harness.new_panel(
                    settings,
                    async_runtime.clone(),
                    viewer_endpoint,
                    cache_dir,
                    is_in_rerun_workspace,
                )
            }
            _ => {{
                let _ = (viewer_endpoint, cache_dir, is_in_rerun_workspace);
                let mut panel = AgentPanel::new(settings, async_runtime.clone());
                panel.set_context(re_agent::SessionContext {
                    model_api_preamble: Some(preamble::model_api_text()),
                    ..Default::default()
                });
                panel
            }}
        };

        panel.set_feedback_enabled(true);

        let (calls_tx, calls_rx) = viewer_toolbox::calls_channel();
        self.viewer_control_calls = Some(calls_rx);
        panel.set_toolbox(viewer_toolbox::ViewerToolbox::new(
            calls_tx,
            egui_ctx.ui_waker(),
        ));

        self.panel = Some(panel);
    }
}

#[cfg(all(test, agent_harness_backend))]
mod tests {
    use super::*;

    #[test]
    fn only_a_development_build_is_told_to_hold_back() {
        let checkout = std::path::PathBuf::from("/home/someone/rerun");
        let directories = [checkout.clone()];
        let text = preamble::Preamble {
            off_limits_directories: &directories,
            ..Default::default()
        }
        .text();
        assert!(text.contains("Do NOT read the source code of Rerun"));
        // The request names the directory the breach report is measured against:
        assert!(text.contains(&checkout.display().to_string()));

        assert!(
            !preamble::Preamble::default()
                .text()
                .contains("Do NOT read the source code of Rerun")
        );
    }

    #[test]
    #[cfg(unix)]
    fn preamble_snapshot() {
        let text = preamble::Preamble {
            agent_dir: Some(Path::new("/opt/rerun-agent")),
            ..Default::default()
        }
        .text();
        insta::assert_snapshot!("preamble", text);
    }
}
