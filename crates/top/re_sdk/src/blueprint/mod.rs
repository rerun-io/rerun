//! Blueprint API for declarative viewer configuration.

mod api;
mod container;
mod panel;
mod view;

pub use api::{Blueprint, BlueprintActivation, BlueprintOpts};
pub use container::{ContainerLike, Grid, Horizontal, Tabs, Vertical};
pub use panel::{BlueprintPanel, SelectionPanel, TimePanel};
pub use view::{MapView, Spatial2DView, Spatial3DView, TextDocumentView, TimeSeriesView, View};
