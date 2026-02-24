//! Blueprint API for configuring viewer layouts.

use re_log_types::{BlueprintActivationCommand, LogMsg};
use re_sdk_types::blueprint::archetypes::ViewportBlueprint;
use re_sdk_types::blueprint::components::{AutoLayout, AutoViews, RootContainer};
use re_sdk_types::datatypes::Bool;

use crate::{RecordingStream, RecordingStreamBuilder, RecordingStreamResult};

use super::{BlueprintPanel, ContainerLike, SelectionPanel, Tabs, TimePanel};

/// Activation options for a [`Blueprint`].
#[derive(Debug, Clone, Copy)]
pub struct BlueprintActivation {
    /// Activate the blueprint immediately in the viewer.
    pub make_active: bool,

    /// Set this blueprint as the default for the application.
    pub make_default: bool,
}

impl Default for BlueprintActivation {
    fn default() -> Self {
        Self {
            make_active: true,
            make_default: true,
        }
    }
}

/// A [`Blueprint`] bundled with its activation options.
#[derive(Debug)]
pub struct BlueprintOpts {
    /// The [`Blueprint`] to send.
    pub blueprint: Blueprint,

    /// How to activate the blueprint.
    pub activation: BlueprintActivation,
}

/// Blueprint for configuring the viewer layout.
#[derive(Debug, Default)]
pub struct Blueprint {
    root_container: Option<ContainerLike>,
    auto_layout: Option<bool>,
    auto_views: Option<bool>,
    blueprint_panel: Option<BlueprintPanel>,
    selection_panel: Option<SelectionPanel>,
    time_panel: Option<TimePanel>,
}

impl Blueprint {
    /// Create a new blueprint with the given root container.
    pub fn new(root: impl Into<ContainerLike>) -> Self {
        let root_like = root.into();
        let root_container = match root_like {
            ContainerLike::Horizontal(_)
            | ContainerLike::Vertical(_)
            | ContainerLike::Grid(_)
            | ContainerLike::Tabs(_) => Some(root_like),
            ContainerLike::View(view) => {
                // Wrap a single view in a Tabs container (matching Python's behavior)
                Some(Tabs::new([view.into()]).into())
            }
        };

        Self {
            root_container,
            ..Self::default()
        }
    }

    /// Create an auto blueprint with automatic layout and view creation.
    pub fn auto() -> Self {
        Self::default().with_auto_views(true).with_auto_layout(true)
    }

    /// Enable or disable automatic layout.
    pub fn with_auto_layout(mut self, enabled: bool) -> Self {
        self.auto_layout = Some(enabled);
        self
    }

    /// Enable or disable automatic view creation.
    pub fn with_auto_views(mut self, enabled: bool) -> Self {
        self.auto_views = Some(enabled);
        self
    }

    /// Configure the blueprint panel.
    pub fn with_blueprint_panel(mut self, panel: BlueprintPanel) -> Self {
        self.blueprint_panel = Some(panel);
        self
    }

    /// Configure the selection panel.
    pub fn with_selection_panel(mut self, panel: SelectionPanel) -> Self {
        self.selection_panel = Some(panel);
        self
    }

    /// Configure the time panel.
    pub fn with_time_panel(mut self, panel: TimePanel) -> Self {
        self.time_panel = Some(panel);
        self
    }

    /// Convert the blueprint into a vector of `LogMsgs`.
    pub(crate) fn to_log_msgs(&self, application_id: &str) -> RecordingStreamResult<Vec<LogMsg>> {
        let (rec, storage) = RecordingStreamBuilder::new(application_id)
            .recording_id(re_log_types::RecordingId::random())
            .blueprint()
            .memory()?;

        // Required for viewer to identify blueprint data
        rec.set_time_sequence("blueprint", 0);

        let mut viewport = ViewportBlueprint::new();

        if let Some(ref root) = self.root_container {
            root.log_to_stream(&rec)?;

            let root_id = match root {
                ContainerLike::Horizontal(h) => h.0.id,
                ContainerLike::Vertical(v) => v.0.id,
                ContainerLike::Grid(g) => g.0.id,
                ContainerLike::Tabs(t) => t.0.id,
                ContainerLike::View(_) => {
                    unreachable!("View should have been wrapped in Tabs container in new()")
                }
            };

            viewport = viewport.with_root_container(RootContainer(root_id.into()));
        }

        if let Some(auto_layout) = self.auto_layout {
            viewport = viewport.with_auto_layout(AutoLayout(Bool(auto_layout)));
        }
        if let Some(auto_views) = self.auto_views {
            viewport = viewport.with_auto_views(AutoViews(Bool(auto_views)));
        }

        rec.log("viewport", &viewport)?;

        if let Some(ref panel) = self.blueprint_panel {
            panel.log_to_stream(&rec)?;
        }
        if let Some(ref panel) = self.selection_panel {
            panel.log_to_stream(&rec)?;
        }
        if let Some(ref panel) = self.time_panel {
            panel.log_to_stream(&rec)?;
        }

        let msgs = storage.take();

        Ok(msgs)
    }

    /// Send the blueprint to the given recording stream.
    pub fn send(
        &self,
        recording: &RecordingStream,
        activation: BlueprintActivation,
    ) -> RecordingStreamResult<()> {
        let application_id = recording
            .store_info()
            .map(|info| info.application_id().to_string())
            .unwrap_or_else(|| "rerun_example_app".to_owned());

        let msgs = self.to_log_msgs(&application_id)?;

        let blueprint_id = msgs
            .first()
            .and_then(|msg| match msg {
                LogMsg::SetStoreInfo(info) => Some(info.info.store_id.clone()),
                _ => None,
            })
            .expect("Blueprint should have at least one SetStoreInfo message");

        let activation_cmd = BlueprintActivationCommand {
            blueprint_id: blueprint_id.clone(),
            make_active: activation.make_active,
            make_default: activation.make_default,
        };

        recording.send_blueprint(msgs, activation_cmd);

        Ok(())
    }
}

impl BlueprintOpts {
    /// Send the blueprint to the given recording stream.
    pub fn send(self, recording: &RecordingStream) -> RecordingStreamResult<()> {
        let Self {
            blueprint,
            activation,
        } = self;
        blueprint.send(recording, activation)
    }
}
