//! Blueprint API for declarative viewer configuration.

mod api;
mod container;
mod view;

pub use api::Blueprint;
pub use container::{ContainerLike, Grid, Horizontal, Tabs, Vertical};
pub use view::{MapView, Spatial2DView, Spatial3DView, TextDocumentView, TimeSeriesView, View};
