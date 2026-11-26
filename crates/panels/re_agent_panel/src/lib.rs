//! The agent panel: a chat with an agent that runs next to the viewer and drives it, through
//! `rerun mcp` for an agent harness, or through the viewer-control and catalog tools for a
//! model-API agent.
//!
//! This crate holds everything that ties the agent to the Rerun Viewer: the preambles, the
//! viewer-control toolbox, the harness setup, the embedded skills, feedback, and
//! analytics. Chat UI that any egui app could use goes in [`re_agent_ui`] instead.

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;

use re_agent::AgentSettings;
use re_agent_tools::{AgentToolbox, CombinedToolbox};
use re_agent_ui::AgentPanel;
use re_async::AsyncRuntimeHandle;
use re_redap_client::ConnectionRegistryHandle;
use re_ui::{ContextExt as _, UiExt as _};

mod agent_analytics;
mod embedded;
mod example_prompts;
mod feedback;
#[cfg(agent_harness_backend)]
mod harness;
mod preamble;
mod product_feedback;
mod reporting;
#[cfg(agent_harness_backend)]
mod self_improve;
mod skills_toolbox;
mod viewer_toolbox;

pub use product_feedback::{AgentFeedback, InvalidFeedback};
pub use viewer_toolbox::ViewerControlCall;

/// The MCP server name an agent harness sees the viewer under. Tools show up as
/// `mcp__rerun__<tool>`.
const MCP_SERVER_NAME: &str = "rerun";

/// Where the skills are unpacked inside the agent directory.
#[cfg(agent_harness_backend)]
const SKILLS_SUBDIR: &str = "agent_context/skills";

/// Offered in an empty conversation while a recording is open.
const RECORDING_EXAMPLE_PROMPTS: [&str; 3] = [
    "Set up a blueprint for this recording",
    "Help me fix the errors in this recording",
    "Summarize what is in this recording",
];

/// Offered in an empty conversation while a dataset is open.
const DATASET_EXAMPLE_PROMPTS: [&str; 5] = [
    "Set up a table blueprint with previews",
    "Find interesting analyses we could do with the query SDK",
    "Summarize what is in this dataset",
    "Help me query this dataset from Python",
    "Find the most interesting segments in this dataset",
];

/// Offered in an empty conversation while a table is open.
const TABLE_EXAMPLE_PROMPTS: [&str; 3] = [
    "Summarize what is in this table",
    "Explain the columns of this table",
    "Help me query this table from Python",
];

/// Offered in an empty conversation while no recording, dataset, or table is open.
const OTHER_EXAMPLE_PROMPTS: [&str; 3] = [
    "Help me open a dataset in Rerun",
    "Help me log my own data to Rerun",
    "Show me an example recording",
];

/// What the viewer is showing, which decides the example prompts in an empty conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownContent {
    /// A dataset entry on a Redap server, on any of its tabs.
    Dataset,

    /// A loaded, non-empty recording.
    Recording,

    /// A table entry on a Redap server, or a local table.
    Table,

    /// Anything else, e.g. the welcome screen or the settings.
    Other,
}

/// The contents of an image file, encoded as PNG or JPEG.
pub struct EncodedImage(pub Vec<u8>);

/// Image files to attach to the prompt being composed.
///
/// Shared, because on the web a dropped file can only be read asynchronously.
#[derive(Clone, Default)]
pub struct DroppedImages(Arc<Mutex<Vec<EncodedImage>>>);

impl DroppedImages {
    /// Attaches this image the next time the panel is shown.
    pub fn push(&self, image: EncodedImage) {
        self.0.lock().push(image);
    }

    fn take(&self) -> Vec<EncodedImage> {
        std::mem::take(&mut *self.0.lock())
    }
}

/// The open agent panel, as a place to drop image files onto.
pub struct ImageDropTarget {
    /// Where the panel is on screen.
    pub rect: egui::Rect,

    /// Where to put the contents of image files dropped onto the panel.
    pub images: DroppedImages,
}

/// The agent chat, configured for the viewer it lives in.
#[derive(Default)]
pub struct ViewerAgentPanel {
    /// Created the first time the panel opens, since initialization probes for installed agents.
    panel: Option<AgentPanel>,

    /// Set when the panel is opened, so the prompt input takes focus once there is one.
    focus_input: bool,

    /// The list the example prompts last handed to [`Self::panel`] were picked from.
    example_prompts: &'static [&'static str],

    /// Prompts waiting for the agent panel to be created — for example, prompts from
    /// the command palette submitted before the agent panel has been opened.
    ///
    /// Use `push_back()` to enqueue, and `pop_front()` to get the next pending prompt.
    pending_prompts: VecDeque<String>,

    /// Conversations waiting for the agent panel to be created, each with its model and opening
    /// prompt, see [`Self::start_conversation_with_model`].
    pending_conversations: VecDeque<(re_agent::ModelPreselect, re_agent::Prompt)>,

    /// Contents of image files dropped onto the viewer, to attach to the prompt being composed.
    dropped_images: DroppedImages,

    /// Where the panel was last shown, or `None` if it is closed.
    rect: Option<egui::Rect>,

    #[cfg(agent_harness_backend)]
    harness: harness::Harness,

    reporting: reporting::Reporting,

    /// Viewer-control operations that a model-API agent asked for, to be run on the UI thread.
    /// Set up with the panel.
    viewer_control_calls: Option<crossbeam::channel::Receiver<viewer_toolbox::QueuedCall>>,

    /// The catalog servers the catalog tools list, as set by [`Self::set_known_servers`].
    #[cfg(not(target_arch = "wasm32"))]
    known_servers: Option<re_redap_client::KnownServers>,

    /// The catalog tools of a model-API agent, which follow [`AgentSettings::catalog_access`]
    /// as it changes. Set up with the panel.
    catalog_tools: Option<re_catalog_tools::CatalogTools>,

    // TODO(RR-4859, RR-4853): remove once the catalog server sends an event for every change.
    /// Catalogs that a model-API agent changed, for the viewer to refresh.
    changed_catalogs: Arc<parking_lot::Mutex<BTreeSet<re_uri::Origin>>>,
}

impl ViewerAgentPanel {
    /// The catalog servers that a model-API agent's catalog tools list, if any.
    ///
    /// Only read when the panel is created, so cheap to call every frame.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_known_servers(&mut self, known_servers: Option<&re_redap_client::KnownServers>) {
        if self.panel.is_none() {
            self.known_servers = known_servers.cloned();
        }
    }

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

    /// Takes the catalogs that the agent changed since the last call, so the viewer can refresh
    /// them and show the change.
    ///
    /// Only for a model-API agent: an agent harness changes catalogs through `rerun mcp`, which
    /// refreshes them in the viewer itself.
    pub fn take_changed_catalogs(&self) -> Vec<re_uri::Origin> {
        std::mem::take(&mut *self.changed_catalogs.lock())
            .into_iter()
            .collect()
    }

    /// Queue feedback that an agent sent about Rerun, to be sent.
    ///
    /// Its text is only sent when `share_text` is set, after the panel's agent has redacted it.
    pub fn submit_product_feedback(&mut self, feedback: AgentFeedback, share_text: bool) {
        self.reporting
            .submit_product_feedback(self.panel.as_ref(), feedback, share_text);
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

    /// Start a new conversation that preselects `model_preselect`, and send it `prompt` once its
    /// agent is ready. Buffers it until the panel exists; the caller must open the panel.
    pub fn start_conversation_with_model(
        &mut self,
        model_preselect: re_agent::ModelPreselect,
        prompt: impl Into<re_agent::Prompt>,
    ) {
        let prompt = prompt.into();
        if let Some(panel) = self.panel.as_mut() {
            panel.open_conversation_with_model(model_preselect, prompt);
        } else {
            self.pending_conversations
                .push_back((model_preselect, prompt));
        }
    }

    /// Where to drop image files to attach them to the prompt being composed.
    ///
    /// `None` while the panel is closed.
    pub fn image_drop_target(&self) -> Option<ImageDropTarget> {
        Some(ImageDropTarget {
            rect: self.rect?,
            images: self.dropped_images.clone(),
        })
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
        connection_registry: &ConnectionRegistryHandle,
        cache_dir: Option<&Path>,
        is_in_rerun_workspace: bool,
        shown_content: ShownContent,
    ) {
        re_tracing::profile_function!();
        let was_open = *open;
        if *open && self.panel.is_none() {
            self.initialize(
                ui.ctx(),
                settings.clone(),
                async_runtime,
                connection_registry,
                cache_dir,
                is_in_rerun_workspace,
            );
        }

        let Self {
            panel,
            focus_input,
            example_prompts,
            pending_prompts,
            pending_conversations,
            dropped_images,
            ..
        } = self;
        if let Some(panel) = panel.as_mut() {
            let prompts: &'static [&'static str] = match shown_content {
                ShownContent::Dataset => &DATASET_EXAMPLE_PROMPTS,
                ShownContent::Recording => &RECORDING_EXAMPLE_PROMPTS,
                ShownContent::Table => &TABLE_EXAMPLE_PROMPTS,
                ShownContent::Other => &OTHER_EXAMPLE_PROMPTS,
            };
            if *example_prompts != prompts {
                panel.set_example_prompts(
                    crate::example_prompts::sample(prompts)
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                );
                *example_prompts = prompts;
            }
            for prompt in pending_prompts.drain(..) {
                panel.queue_prompt(prompt);
            }
            for (model_preselect, prompt) in pending_conversations.drain(..) {
                panel.open_conversation_with_model(model_preselect, prompt);
            }
            for EncodedImage(bytes) in dropped_images.take() {
                if let Err(err) = panel.attach_image_file(&bytes) {
                    re_log::warn!("Could not attach the dropped image: {err:#}");
                }
            }
            // A collapsed panel never runs its `ui`, and its agents would otherwise
            // sit on their messages until the user opens it again.
            panel.poll_events();

            if *focus_input {
                panel.request_input_focus();
                *focus_input = false;
            }
        }
        let response = egui::Panel::right("agent_panel")
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
                }
            });

        self.rect = response
            .filter(|_| *open)
            .map(|response| response.response.rect);

        // Dragging the collapsed panel open does not go through `toggle_agent_panel`:
        self.focus_input |= !was_open && *open;

        let Some(panel) = &mut self.panel else {
            return;
        };
        // Also while collapsed, since its conversations keep running and are saved as they go.
        settings.clone_from(panel.settings());
        if let Some(catalog_tools) = &self.catalog_tools {
            catalog_tools.set_access(settings.catalog_access);
        }
        #[cfg(agent_harness_backend)]
        self.harness.handle_self_improve_request(panel);
        // Before the redactors run, so that turning sharing off drops what is queued before any
        // of it can be sent.
        let re_agent::SessionReports {
            turns,
            startups,
            mcp_startup_failures,
        } = panel.take_reports();
        for startup in &startups {
            agent_analytics::record_startup(startup);
        }
        cfg_select! {
            agent_harness_backend => harness::record_mcp_startup_failures(&mcp_startup_failures),
            _ => drop(mcp_startup_failures),
        }
        self.reporting.record_finished_turns(panel, turns);
        self.reporting.ui(panel, ui);
    }

    /// Initialize the inner panel.
    ///
    /// `async_runtime` runs model-API agents, and `egui_ctx` is woken for their tool calls.
    /// `connection_registry` reaches the catalogs for the catalog tools, and its internal catalog
    /// is the viewer an agent harness connects to. `cache_dir` and `is_in_rerun_workspace` only
    /// matter to agent harnesses; see [`harness::Harness::new_panel`].
    fn initialize(
        &mut self,
        egui_ctx: &egui::Context,
        settings: AgentSettings,
        async_runtime: &AsyncRuntimeHandle,
        connection_registry: &ConnectionRegistryHandle,
        cache_dir: Option<&Path>,
        is_in_rerun_workspace: bool,
    ) {
        re_tracing::profile_function!();

        let viewer_endpoint = connection_registry
            .internal_origin()
            .map(|origin| origin.as_url());

        let mut panel = cfg_select! {
            agent_harness_backend => self.harness.new_panel(
                settings,
                async_runtime.clone(),
                viewer_endpoint.as_deref(),
                cache_dir,
                is_in_rerun_workspace,
            ),
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
        let viewer_tools = viewer_toolbox::ViewerToolbox::new(calls_tx, egui_ctx.ui_waker());
        let catalog_tools = re_catalog_tools::CatalogTools::new(
            connection_registry.clone(),
            async_runtime.clone(),
            connection_registry.internal_origin(),
        );
        #[cfg(not(target_arch = "wasm32"))]
        let catalog_tools = catalog_tools.with_known_servers(self.known_servers.clone());
        let changed_catalogs = Arc::clone(&self.changed_catalogs);
        let ui_waker = egui_ctx.ui_waker();
        catalog_tools.on_catalog_change(move |origin| {
            changed_catalogs.lock().insert(origin.clone());
            ui_waker.wake();
        });
        catalog_tools.set_access(panel.settings().catalog_access);
        panel.set_toolbox(model_api_toolbox(viewer_tools, catalog_tools.clone()));
        self.catalog_tools = Some(catalog_tools);

        self.panel = Some(panel);
    }
}

/// The tools of a model-API agent: the viewer-control tools, the catalog tools, and the Rerun
/// skills.
fn model_api_toolbox(
    viewer_tools: Arc<viewer_toolbox::ViewerToolbox>,
    catalog_tools: re_catalog_tools::CatalogTools,
) -> Arc<dyn AgentToolbox> {
    let mut toolboxes: Vec<Arc<dyn AgentToolbox>> = vec![viewer_tools, Arc::new(catalog_tools)];
    if let Some(skills) = skills_toolbox::SkillsToolbox::embedded() {
        toolboxes.push(Arc::new(skills));
    }
    Arc::new(CombinedToolbox::new(
        Some(MCP_SERVER_NAME.to_owned()),
        toolboxes,
    ))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod toolbox_tests {
    use serde_json::Map;

    use super::*;

    #[tokio::test]
    async fn a_model_api_agent_gets_viewer_and_catalog_tools() {
        let (calls_tx, _calls_rx) = viewer_toolbox::calls_channel();
        let catalog_tools = re_catalog_tools::CatalogTools::new(
            re_redap_client::ConnectionRegistry::new_without_stored_credentials(),
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("runtime"),
            None,
        );
        let toolbox = model_api_toolbox(
            viewer_toolbox::ViewerToolbox::new(calls_tx, re_async::UiWaker::noop()),
            catalog_tools.clone(),
        );
        let names: Vec<String> = toolbox
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert!(names.iter().any(|name| name == "viewer_get_state"));
        if cfg!(feature = "agent_skills") {
            assert!(names.iter().any(|name| name == skills_toolbox::SKILL_READ));
        }
        for tool in re_catalog_tools::CatalogTool::available() {
            assert!(names.iter().any(|name| name == tool.name()), "{tool:?}");
        }
        assert!(!toolbox.requires_approval("catalog_query_tables", &Map::new()));
        assert!(toolbox.requires_approval("catalog_connect", &Map::new()));
        assert!(toolbox.requires_approval("catalog_create_datasets", &Map::new()));

        catalog_tools.set_access(re_agent_tools::CatalogAccess::ReadOnly);
        let names: Vec<String> = toolbox
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect();
        assert!(names.iter().any(|name| name == "catalog_list_entries"));
        assert!(!names.iter().any(|name| name == "catalog_create_datasets"));
    }

    #[tokio::test]
    async fn a_model_api_agent_started_read_only_gets_the_write_tools_once_allowed() {
        let (calls_tx, _calls_rx) = viewer_toolbox::calls_channel();
        let catalog_tools = re_catalog_tools::CatalogTools::new(
            re_redap_client::ConnectionRegistry::new_without_stored_credentials(),
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("runtime"),
            None,
        );
        catalog_tools.set_access(re_agent_tools::CatalogAccess::ReadOnly);
        let toolbox = model_api_toolbox(
            viewer_toolbox::ViewerToolbox::new(calls_tx, re_async::UiWaker::noop()),
            catalog_tools.clone(),
        );
        let offers_create = || {
            toolbox
                .definitions()
                .iter()
                .any(|definition| definition.name == "catalog_create_datasets")
        };
        assert!(!offers_create());

        catalog_tools.set_access(re_agent_tools::CatalogAccess::ReadWrite);
        assert!(offers_create());
        assert!(toolbox.requires_approval("catalog_create_datasets", &Map::new()));
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
