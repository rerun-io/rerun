use std::fmt::Debug;

use ahash::HashMap;
use egui_tiles::TileId;
use re_chunk::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
use re_sdk_types::blueprint::components::{
    ActiveTab, ColumnShare, ContainerKind, GridColumns, IncludedContent, RowShare,
};
use re_sdk_types::components::{Name, Visible};
use re_sdk_types::{Archetype as _, Loggable as _};
use re_viewer_context::{
    BlueprintContext as _, ContainerId, Contents, ContentsName, ViewId, ViewerContext,
};

/// The native version of a [`re_sdk_types::blueprint::archetypes::ContainerBlueprint`].
///
/// This represents a single container in the blueprint. On each frame, it is
/// used to populate an [`egui_tiles::Container`]. Each child in `contents` can
/// be either a [`ViewId`] or another [`ContainerId`].
///
/// The main reason this exists is to handle type conversions that aren't yet
/// well handled by the code-generated archetypes.
#[derive(Clone, Debug)]
pub struct ContainerBlueprint {
    pub id: ContainerId,
    pub container_kind: egui_tiles::ContainerKind,
    pub display_name: Option<String>,
    pub contents: Vec<Contents>,
    pub col_shares: Vec<f32>,
    pub row_shares: Vec<f32>,
    pub active_tab: Option<Contents>,
    pub visible: bool,
    pub grid_columns: Option<u32>,
}

impl Default for ContainerBlueprint {
    fn default() -> Self {
        Self {
            id: ContainerId::random(),
            container_kind: egui_tiles::ContainerKind::Grid,
            display_name: None,
            contents: vec![],
            col_shares: vec![],
            row_shares: vec![],
            active_tab: None,
            visible: true,
            grid_columns: None,
        }
    }
}

impl ContainerBlueprint {
    pub fn new(id: ContainerId) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    /// Attempt to load a [`ContainerBlueprint`] from the blueprint store.
    pub fn try_from_db(
        blueprint_db: &EntityDb,
        query: &LatestAtQuery,
        id: ContainerId,
    ) -> Option<Self> {
        re_tracing::profile_function!();

        // ----

        let results = blueprint_db.storage_engine().cache().latest_at(
            query,
            &id.as_entity_path(),
            blueprint_archetypes::ContainerBlueprint::all_component_identifiers(),
        );

        // This is a required component. Note that when loading containers we crawl the subtree and so
        // cleared empty container paths may exist transiently. The fact that they have an empty container_kind
        // is the marker that the have been cleared and not an error.
        let container_kind = results.component_mono::<ContainerKind>(
            blueprint_archetypes::ContainerBlueprint::descriptor_container_kind().component,
        )?;

        let display_name = results.component_mono::<Name>(
            blueprint_archetypes::ContainerBlueprint::descriptor_display_name().component,
        );
        let contents = results.component_batch::<IncludedContent>(
            blueprint_archetypes::ContainerBlueprint::descriptor_contents().component,
        );
        let col_shares = results.component_batch::<ColumnShare>(
            blueprint_archetypes::ContainerBlueprint::descriptor_col_shares().component,
        );
        let row_shares = results.component_batch::<RowShare>(
            blueprint_archetypes::ContainerBlueprint::descriptor_row_shares().component,
        );
        let active_tab = results.component_mono::<ActiveTab>(
            blueprint_archetypes::ContainerBlueprint::descriptor_active_tab().component,
        );
        let visible = results.component_mono::<Visible>(
            blueprint_archetypes::ContainerBlueprint::descriptor_visible().component,
        );
        let grid_columns = results.component_mono::<GridColumns>(
            blueprint_archetypes::ContainerBlueprint::descriptor_grid_columns().component,
        );

        // ----

        let container_kind = crate::container_kind_to_egui(container_kind);
        let display_name = display_name.map(|v| v.0.to_string());

        let contents = contents
            .unwrap_or_default()
            .iter()
            .filter_map(|id| Contents::try_from(&id.0.clone().into()))
            .collect();

        let col_shares = col_shares
            .unwrap_or_default()
            .iter()
            .map(|v| *v.0)
            .collect();
        let row_shares = row_shares
            .unwrap_or_default()
            .iter()
            .map(|v| *v.0)
            .collect();

        let active_tab = active_tab.and_then(|id| Contents::try_from(&id.0.into()));

        let visible = visible.is_none_or(|v| **v);
        let grid_columns = grid_columns.map(|v| **v);

        Some(Self {
            id,
            container_kind,
            display_name,
            contents,
            col_shares,
            row_shares,
            active_tab,
            visible,
            grid_columns,
        })
    }

    pub fn entity_path(&self) -> EntityPath {
        self.id.as_entity_path()
    }

    pub fn add_child(&mut self, content: Contents) {
        self.contents.push(content);
        match self.container_kind {
            egui_tiles::ContainerKind::Tabs => {
                self.active_tab = self.active_tab.or(Some(content));
            }
            egui_tiles::ContainerKind::Horizontal => {
                self.col_shares.push(1.0);
            }
            egui_tiles::ContainerKind::Vertical => {
                self.row_shares.push(1.0);
            }
            egui_tiles::ContainerKind::Grid => {
                // dunno
            }
        }
    }

    /// Persist the entire [`ContainerBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`ContainerBlueprint`] was created with [`Self::from_egui_tiles_container`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        let Self {
            id,
            container_kind,
            display_name,
            contents,
            col_shares,
            row_shares,
            active_tab,
            visible,
            grid_columns,
        } = self;

        let contents: Vec<_> = contents.iter().map(|item| item.as_entity_path()).collect();

        let container_kind = crate::container_kind_from_egui(*container_kind);
        let mut arch = re_sdk_types::blueprint::archetypes::ContainerBlueprint::new(container_kind)
            .with_contents(&contents)
            .with_col_shares(col_shares.clone())
            .with_row_shares(row_shares.clone())
            .with_visible(*visible);

        // Note: it's important to _not_ clear the `Name` component if `display_name` is set to
        // `None`, as we call this function with `ContainerBlueprint` recreated from `egui_tiles`,
        // which is lossy with custom names.
        if let Some(display_name) = display_name {
            arch = arch.with_display_name(display_name.clone());
        }

        // We want to write an empty array if `active_tab` is none. So can't use `arch.with_active_tab`
        // here.
        arch.active_tab =
            re_sdk_types::try_serialize_field::<re_sdk_types::blueprint::components::ActiveTab>(
                re_sdk_types::blueprint::archetypes::ContainerBlueprint::descriptor_active_tab(),
                active_tab.map(|c| c.as_entity_path()).as_ref(),
            );

        if let Some(cols) = grid_columns {
            arch = arch.with_grid_columns(*cols);
        } else {
            arch.grid_columns = Some(re_sdk_types::SerializedComponentBatch::new(
                re_sdk_types::blueprint::components::GridColumns::arrow_empty(),
                re_sdk_types::blueprint::archetypes::ContainerBlueprint::descriptor_grid_columns(),
            ));
        }

        ctx.save_blueprint_archetype(id.as_entity_path(), &arch);
    }

    /// Creates a new [`ContainerBlueprint`] from the given [`egui_tiles::Container`].
    ///
    /// This [`ContainerBlueprint`] is ephemeral. If you want to make it permanent you
    /// must call [`Self::save_to_blueprint_store`].
    pub fn from_egui_tiles_container(
        container_id: ContainerId,
        container: &egui_tiles::Container,
        visible: bool,
        tile_to_contents: &HashMap<TileId, Contents>,
    ) -> Self {
        let contents = container
            .children()
            .filter_map(|child_id| {
                tile_to_contents.get(child_id).copied().or_else(|| {
                    re_log::warn_once!("Missing child when building container.");
                    None
                })
            })
            .collect();

        match container {
            egui_tiles::Container::Tabs(tab) => {
                let active_tab = tab.active.and_then(|id| tile_to_contents.get(&id).copied());

                Self {
                    id: container_id,
                    container_kind: egui_tiles::ContainerKind::Tabs,
                    display_name: None, // keep whatever name is already set
                    contents,
                    col_shares: vec![],
                    row_shares: vec![],
                    active_tab,
                    visible,
                    grid_columns: None,
                }
            }
            egui_tiles::Container::Linear(linear) => match linear.dir {
                egui_tiles::LinearDir::Horizontal => {
                    let kind = egui_tiles::ContainerKind::Horizontal;
                    Self {
                        id: container_id,
                        container_kind: kind,
                        display_name: None, // keep whatever name is already set
                        contents,
                        col_shares: linear
                            .children
                            .iter()
                            .map(|child| linear.shares[*child])
                            .collect(),
                        row_shares: vec![],
                        active_tab: None,
                        visible,
                        grid_columns: None,
                    }
                }
                egui_tiles::LinearDir::Vertical => {
                    let kind = egui_tiles::ContainerKind::Vertical;
                    Self {
                        id: container_id,
                        container_kind: kind,
                        display_name: None, // keep whatever name is already set
                        contents,
                        col_shares: vec![],
                        row_shares: linear
                            .children
                            .iter()
                            .map(|child| linear.shares[*child])
                            .collect(),
                        active_tab: None,
                        visible,
                        grid_columns: None,
                    }
                }
            },
            egui_tiles::Container::Grid(grid) => Self {
                id: container_id,
                container_kind: egui_tiles::ContainerKind::Grid,
                display_name: None, // keep whatever name is already set
                contents,
                col_shares: grid.col_shares.clone(),
                row_shares: grid.row_shares.clone(),
                active_tab: None,
                visible,
                grid_columns: match grid.layout {
                    egui_tiles::GridLayout::Columns(cols) => Some(cols as u32),
                    egui_tiles::GridLayout::Auto => None,
                },
            },
        }
    }

    /// Placeholder name displayed in the UI if the user hasn't explicitly named the view.
    #[inline]
    pub fn missing_name_placeholder(&self) -> String {
        match self.container_kind {
            egui_tiles::ContainerKind::Tabs => "Tab container",
            egui_tiles::ContainerKind::Horizontal => "Horizontal container",
            egui_tiles::ContainerKind::Vertical => "Vertical container",
            egui_tiles::ContainerKind::Grid => "Grid container",
        }
        .to_owned()
    }

    /// Returns this container's display name
    ///
    /// When returning [`ContentsName::Placeholder`], the UI should display the resulting name using
    /// `re_ui::LabelStyle::Unnamed`.
    #[inline]
    pub fn display_name_or_default(&self) -> ContentsName {
        self.display_name.clone().map_or_else(
            || ContentsName::Placeholder(self.missing_name_placeholder()),
            ContentsName::Named,
        )
    }

    /// Sets the display name for this container.
    #[inline]
    pub fn set_display_name(&self, ctx: &ViewerContext<'_>, name: Option<String>) {
        if name != self.display_name {
            match name {
                Some(name) => {
                    let component = Name(name.into());
                    ctx.save_blueprint_component(
                        self.entity_path(),
                        &blueprint_archetypes::ContainerBlueprint::descriptor_display_name(),
                        &component,
                    );
                }
                None => {
                    ctx.clear_blueprint_component(
                        self.entity_path(),
                        blueprint_archetypes::ContainerBlueprint::descriptor_display_name(),
                    );
                }
            }
        }
    }

    #[inline]
    pub fn set_visible(&self, ctx: &ViewerContext<'_>, visible: bool) {
        if visible != self.visible {
            let component = Visible::from(visible);
            ctx.save_blueprint_component(
                self.entity_path(),
                &blueprint_archetypes::ContainerBlueprint::descriptor_visible(),
                &component,
            );
        }
    }

    #[inline]
    pub fn set_grid_columns(&self, ctx: &ViewerContext<'_>, grid_columns: Option<u32>) {
        if grid_columns != self.grid_columns {
            if let Some(grid_columns) = grid_columns {
                let component = GridColumns(grid_columns.into());
                ctx.save_blueprint_component(
                    self.entity_path(),
                    &blueprint_archetypes::ContainerBlueprint::descriptor_grid_columns(),
                    &component,
                );
            } else {
                ctx.clear_blueprint_component(
                    self.entity_path(),
                    blueprint_archetypes::ContainerBlueprint::descriptor_grid_columns(),
                );
            }
        }
    }

    /// Clears the blueprint component for this container.
    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        // We can't delete the entity, because we need to support undo.
        // TODO(#8249): configure blueprint GC to remove this entity if all that remains is the recursive clear.
        ctx.save_blueprint_archetype(
            self.entity_path(),
            &re_sdk_types::archetypes::Clear::recursive(),
        );
    }

    pub fn to_tile(&self) -> egui_tiles::Tile<ViewId> {
        let children = self
            .contents
            .iter()
            .map(|item| item.as_tile_id())
            .collect::<Vec<_>>();

        let container = match self.container_kind {
            egui_tiles::ContainerKind::Tabs => {
                let mut tabs = egui_tiles::Tabs::new(children);
                tabs.active = self.active_tab.as_ref().map(|id| id.as_tile_id());
                egui_tiles::Container::Tabs(tabs)
            }
            egui_tiles::ContainerKind::Horizontal | egui_tiles::ContainerKind::Vertical => {
                match self.container_kind {
                    egui_tiles::ContainerKind::Horizontal => {
                        let mut linear = egui_tiles::Linear::new(
                            egui_tiles::LinearDir::Horizontal,
                            children.clone(),
                        );

                        for (share, id) in self.col_shares.iter().zip(children.iter()) {
                            linear.shares.set_share(*id, *share);
                        }

                        egui_tiles::Container::Linear(linear)
                    }
                    egui_tiles::ContainerKind::Vertical => {
                        let mut linear = egui_tiles::Linear::new(
                            egui_tiles::LinearDir::Vertical,
                            children.clone(),
                        );

                        for (share, id) in self.row_shares.iter().zip(children.iter()) {
                            linear.shares.set_share(*id, *share);
                        }

                        egui_tiles::Container::Linear(linear)
                    }
                    _ => unreachable!(),
                }
            }
            egui_tiles::ContainerKind::Grid => {
                let mut grid = egui_tiles::Grid::new(children);

                grid.col_shares.clone_from(&self.col_shares);
                grid.row_shares.clone_from(&self.row_shares);

                if let Some(cols) = self.grid_columns {
                    grid.layout = egui_tiles::GridLayout::Columns(cols as usize);
                } else {
                    grid.layout = egui_tiles::GridLayout::Auto;
                }

                egui_tiles::Container::Grid(grid)
            }
        };

        egui_tiles::Tile::Container(container)
    }
}
