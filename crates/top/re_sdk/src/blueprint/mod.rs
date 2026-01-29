//! Blueprint API for declarative viewer configuration.

mod api;
mod container;
mod panel;
mod view;

pub use api::{Blueprint, BlueprintActivation, BlueprintOpts};
pub use container::{ContainerLike, Grid, Horizontal, Tabs, Vertical};
pub use panel::{BlueprintPanel, SelectionPanel, TimePanel};
pub use view::{MapView, Spatial2DView, Spatial3DView, TextDocumentView, TimeSeriesView, View};

// Re-export types for working with visualizers and component mappings
pub use re_sdk_types::blueprint::datatypes::{ComponentSourceKind, VisualizerComponentMapping};
pub use re_sdk_types::{VisualizableArchetype, Visualizer};

/// Components used exclusively in blueprint stores.
pub mod components {
    pub use re_sdk_types::blueprint::components::*;
}

/// Datatypes used exclusively in blueprint stores.
pub mod datatypes {
    pub use re_sdk_types::blueprint::datatypes::*;
}
