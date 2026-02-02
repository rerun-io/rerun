//! Container types for blueprint layouts.

use uuid::Uuid;

use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::ContainerBlueprint;
use re_sdk_types::blueprint::components::{ColumnShare, ContainerKind, IncludedContent, RowShare};
use re_sdk_types::components::{Name, Visible};
use re_sdk_types::datatypes::{Bool, Float32};

/// Internal container data shared by all container types.
#[derive(Debug)]
pub(crate) struct Container {
    pub(crate) id: Uuid,
    pub(crate) kind: ContainerKind,
    pub(crate) contents: Vec<ContainerLike>,
    pub(crate) name: Option<String>,
    pub(crate) visible: Option<bool>,
    pub(crate) column_shares: Option<Vec<f32>>,
    pub(crate) row_shares: Option<Vec<f32>>,
    pub(crate) grid_columns: Option<u32>,
    pub(crate) active_tab: Option<String>,
}

/// A horizontal container that arranges children left-to-right.
#[derive(Debug)]
pub struct Horizontal(pub(crate) Container);

/// A vertical container that arranges children top-to-bottom.
#[derive(Debug)]
pub struct Vertical(pub(crate) Container);

/// A tabs container that shows children as switchable tabs.
#[derive(Debug)]
pub struct Tabs(pub(crate) Container);

/// A grid container that lays out children in a grid.
#[derive(Debug)]
pub struct Grid(pub(crate) Container);

/// Types that can be contained in a container.
#[derive(Debug)]
pub enum ContainerLike {
    /// A horizontal container.
    Horizontal(Horizontal),

    /// A vertical container.
    Vertical(Vertical),

    /// A grid container.
    Grid(Grid),

    /// A tabs container.
    Tabs(Tabs),

    /// A view.
    View(crate::blueprint::View),
}

pub(crate) trait AsContainer {
    fn as_container(&self) -> &Container;
}

impl AsContainer for Horizontal {
    fn as_container(&self) -> &Container {
        &self.0
    }
}

impl AsContainer for Vertical {
    fn as_container(&self) -> &Container {
        &self.0
    }
}

impl AsContainer for Grid {
    fn as_container(&self) -> &Container {
        &self.0
    }
}

impl AsContainer for Tabs {
    fn as_container(&self) -> &Container {
        &self.0
    }
}

impl ContainerLike {
    /// Get the blueprint path for this container or view.
    pub(crate) fn blueprint_path(&self) -> EntityPath {
        match self {
            Self::Horizontal(c) => format!("container/{}", c.0.id).into(),
            Self::Vertical(c) => format!("container/{}", c.0.id).into(),
            Self::Grid(c) => format!("container/{}", c.0.id).into(),
            Self::Tabs(c) => format!("container/{}", c.0.id).into(),
            Self::View(v) => v.blueprint_path(),
        }
    }

    /// Log this container or view to the blueprint stream.
    pub(crate) fn log_to_stream(
        &self,
        stream: &crate::RecordingStream,
    ) -> crate::RecordingStreamResult<()> {
        match self {
            Self::Horizontal(h) => log_container(h.as_container(), stream),
            Self::Vertical(v) => log_container(v.as_container(), stream),
            Self::Grid(g) => log_container(g.as_container(), stream),
            Self::Tabs(t) => log_container(t.as_container(), stream),
            Self::View(v) => v.log_to_stream(stream),
        }
    }
}

/// Helper function to log a container.
fn log_container(
    container: &Container,
    stream: &crate::RecordingStream,
) -> crate::RecordingStreamResult<()> {
    for child in &container.contents {
        child.log_to_stream(stream)?;
    }

    let mut arch = ContainerBlueprint::new(container.kind);

    if let Some(ref name) = container.name {
        arch = arch.with_display_name(Name(name.clone().into()));
    }

    let child_paths: Vec<IncludedContent> = container
        .contents
        .iter()
        .map(|child| IncludedContent(child.blueprint_path().to_string().into()))
        .collect();

    if !child_paths.is_empty() {
        arch = arch.with_contents(child_paths);
    }

    if let Some(ref col_shares) = container.column_shares {
        arch = arch.with_col_shares(col_shares.iter().map(|&s| ColumnShare(Float32(s))));
    }

    if let Some(ref row_shares) = container.row_shares {
        arch = arch.with_row_shares(row_shares.iter().map(|&s| RowShare(Float32(s))));
    }

    if let Some(visible) = container.visible {
        arch = arch.with_visible(Visible(Bool(visible)));
    }

    if let Some(grid_columns) = container.grid_columns {
        arch = arch.with_grid_columns(re_sdk_types::blueprint::components::GridColumns(
            re_sdk_types::datatypes::UInt32(grid_columns),
        ));
    }

    if let Some(ref active_tab) = container.active_tab {
        arch = arch.with_active_tab(re_sdk_types::blueprint::components::ActiveTab(
            re_sdk_types::datatypes::EntityPath::from(active_tab.as_str()),
        ));
    }

    let path: EntityPath = format!("container/{}", container.id).into();
    stream.log(path, &arch)
}

impl Horizontal {
    /// Create a new horizontal container with the given contents.
    pub fn new(contents: impl IntoIterator<Item = ContainerLike>) -> Self {
        Self(Container {
            id: Uuid::new_v4(),
            kind: ContainerKind::Horizontal,
            contents: contents.into_iter().collect(),
            name: None,
            visible: None,
            column_shares: None,
            row_shares: None,
            grid_columns: None,
            active_tab: None,
        })
    }

    /// Set the name of this container.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.0.name = Some(name.into());
        self
    }

    /// Set the column shares for layout.
    pub fn with_column_shares(mut self, shares: impl Into<Vec<f32>>) -> Self {
        self.0.column_shares = Some(shares.into());
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}

impl Vertical {
    /// Create a new vertical container with the given contents.
    pub fn new(contents: impl IntoIterator<Item = ContainerLike>) -> Self {
        Self(Container {
            id: Uuid::new_v4(),
            kind: ContainerKind::Vertical,
            contents: contents.into_iter().collect(),
            name: None,
            visible: None,
            column_shares: None,
            row_shares: None,
            grid_columns: None,
            active_tab: None,
        })
    }

    /// Set the name of this container.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.0.name = Some(name.into());
        self
    }

    /// Set the row shares for layout.
    pub fn with_row_shares(mut self, shares: impl Into<Vec<f32>>) -> Self {
        self.0.row_shares = Some(shares.into());
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }
}

impl Tabs {
    /// Create a new tabs container with the given contents.
    pub fn new(contents: impl IntoIterator<Item = ContainerLike>) -> Self {
        Self(Container {
            id: Uuid::new_v4(),
            kind: ContainerKind::Tabs,
            contents: contents.into_iter().collect(),
            name: None,
            visible: None,
            column_shares: None,
            row_shares: None,
            grid_columns: None,
            active_tab: None,
        })
    }

    /// Set the name of this container.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.0.name = Some(name.into());
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }

    /// Set the active tab by entity path.
    pub fn with_active_tab(mut self, path: impl Into<String>) -> Self {
        self.0.active_tab = Some(path.into());
        self
    }
}

impl Grid {
    /// Create a new grid container with the given contents.
    pub fn new(contents: impl IntoIterator<Item = ContainerLike>) -> Self {
        Self(Container {
            id: Uuid::new_v4(),
            kind: ContainerKind::Grid,
            contents: contents.into_iter().collect(),
            name: None,
            visible: None,
            column_shares: None,
            row_shares: None,
            grid_columns: None,
            active_tab: None,
        })
    }

    /// Set the name of this container.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.0.name = Some(name.into());
        self
    }

    /// Set column shares for relative sizing.
    pub fn with_column_shares(mut self, shares: impl IntoIterator<Item = f32>) -> Self {
        self.0.column_shares = Some(shares.into_iter().collect());
        self
    }

    /// Set row shares for relative sizing.
    pub fn with_row_shares(mut self, shares: impl IntoIterator<Item = f32>) -> Self {
        self.0.row_shares = Some(shares.into_iter().collect());
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.0.visible = Some(visible);
        self
    }

    /// Set the number of columns for the grid layout.
    pub fn with_grid_columns(mut self, columns: u32) -> Self {
        self.0.grid_columns = Some(columns);
        self
    }
}

impl From<Horizontal> for ContainerLike {
    fn from(c: Horizontal) -> Self {
        Self::Horizontal(c)
    }
}

impl From<Vertical> for ContainerLike {
    fn from(c: Vertical) -> Self {
        Self::Vertical(c)
    }
}

impl From<Grid> for ContainerLike {
    fn from(c: Grid) -> Self {
        Self::Grid(c)
    }
}

impl From<Tabs> for ContainerLike {
    fn from(c: Tabs) -> Self {
        Self::Tabs(c)
    }
}

impl From<crate::blueprint::View> for ContainerLike {
    fn from(view: crate::blueprint::View) -> Self {
        Self::View(view)
    }
}

impl From<crate::blueprint::TimeSeriesView> for ContainerLike {
    fn from(view: crate::blueprint::TimeSeriesView) -> Self {
        Self::View(view.0)
    }
}

impl From<crate::blueprint::MapView> for ContainerLike {
    fn from(view: crate::blueprint::MapView) -> Self {
        Self::View(view.0)
    }
}

impl From<crate::blueprint::TextDocumentView> for ContainerLike {
    fn from(view: crate::blueprint::TextDocumentView) -> Self {
        Self::View(view.0)
    }
}

impl From<crate::blueprint::Spatial2DView> for ContainerLike {
    fn from(view: crate::blueprint::Spatial2DView) -> Self {
        Self::View(view.0)
    }
}

impl From<crate::blueprint::Spatial3DView> for ContainerLike {
    fn from(view: crate::blueprint::Spatial3DView) -> Self {
        Self::View(view.0)
    }
}
