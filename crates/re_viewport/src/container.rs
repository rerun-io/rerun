use ahash::HashMap;
use egui_tiles::TileId;
use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log::ResultExt;
use re_log_types::{DataRow, EntityPath, RowId};
use re_query::query_archetype;
use re_types::blueprint::components::Visible;
use re_types_core::{archetypes::Clear, ArrowBuffer};
use re_viewer_context::{
    blueprint_timepoint_for_writes, BlueprintId, BlueprintIdRegistry, ContainerId, Item,
    SpaceViewId, SystemCommand, SystemCommandSender as _, ViewerContext,
};

use crate::blueprint::components::GridColumns;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    Container(ContainerId),
    SpaceView(SpaceViewId),
}

impl Contents {
    fn try_from(path: &EntityPath) -> Option<Self> {
        if path.starts_with(SpaceViewId::registry()) {
            Some(Self::SpaceView(SpaceViewId::from_entity_path(path)))
        } else if path.starts_with(ContainerId::registry()) {
            Some(Self::Container(ContainerId::from_entity_path(path)))
        } else {
            None
        }
    }

    #[inline]
    fn as_entity_path(&self) -> EntityPath {
        match self {
            Self::Container(id) => id.as_entity_path(),
            Self::SpaceView(id) => id.as_entity_path(),
        }
    }

    #[inline]
    pub fn as_tile_id(&self) -> TileId {
        match self {
            Self::Container(id) => blueprint_id_to_tile_id(id),
            Self::SpaceView(id) => blueprint_id_to_tile_id(id),
        }
    }

    #[inline]
    pub fn as_item(&self) -> Item {
        match self {
            Contents::Container(container_id) => Item::Container(*container_id),
            Contents::SpaceView(space_view_id) => Item::SpaceView(*space_view_id),
        }
    }

    #[inline]
    pub fn as_container_id(&self) -> Option<ContainerId> {
        match self {
            Self::Container(id) => Some(*id),
            Self::SpaceView(_) => None,
        }
    }

    #[inline]
    pub fn as_space_view_id(&self) -> Option<SpaceViewId> {
        match self {
            Self::SpaceView(id) => Some(*id),
            Self::Container(_) => None,
        }
    }
}

#[inline]
pub fn blueprint_id_to_tile_id<T: BlueprintIdRegistry>(id: &BlueprintId<T>) -> TileId {
    TileId::from_u64(id.hash())
}

impl From<SpaceViewId> for Contents {
    #[inline]
    fn from(id: SpaceViewId) -> Self {
        Self::SpaceView(id)
    }
}

impl From<ContainerId> for Contents {
    #[inline]
    fn from(id: ContainerId) -> Self {
        Self::Container(id)
    }
}

/// The native version of a [`crate::blueprint::archetypes::ContainerBlueprint`].
///
/// This represents a single container in the blueprint. On each frame, it is
/// used to populate an [`egui_tiles::Container`]. Each child in `contents` can
/// be either a [`SpaceViewId`] or another [`ContainerId`].
///
/// The main reason this exists is to handle type conversions that aren't yet
/// well handled by the code-generated archetypes.
#[derive(Debug)]
pub struct ContainerBlueprint {
    pub id: ContainerId,
    pub container_kind: egui_tiles::ContainerKind,
    pub display_name: String,
    pub contents: Vec<Contents>,
    pub col_shares: Vec<f32>,
    pub row_shares: Vec<f32>,
    pub active_tab: Option<Contents>,
    pub visible: bool,
    pub grid_columns: Option<u32>,
}

impl ContainerBlueprint {
    /// Attempt to load a [`ContainerBlueprint`] from the blueprint store.
    pub fn try_from_db(
        blueprint_db: &EntityDb,
        query: &LatestAtQuery,
        id: ContainerId,
    ) -> Option<Self> {
        re_tracing::profile_function!();

        let crate::blueprint::archetypes::ContainerBlueprint {
            container_kind,
            display_name,
            contents,
            col_shares,
            row_shares,
            active_tab,
            visible,
            grid_columns,
        } = query_archetype(blueprint_db.store(), query, &id.as_entity_path())
            .and_then(|arch| arch.to_archetype())
            .map_err(|err| {
                if !matches!(err, re_query::QueryError::PrimaryNotFound(_)) {
                    if cfg!(debug_assertions) {
                        re_log::error!("Failed to load Container blueprint: {err}.");
                    } else {
                        re_log::debug!("Failed to load Container blueprint: {err}.");
                    }
                }
            })
            .ok()?;

        let container_kind = container_kind.into();

        // TODO(jleibs): Don't use debug print for this
        let display_name =
            display_name.map_or_else(|| format!("{container_kind:?}"), |v| v.0.to_string());

        let contents = contents
            .unwrap_or_default()
            .0
            .into_iter()
            .filter_map(|id| Contents::try_from(&id.into()))
            .collect();

        let col_shares = col_shares.unwrap_or_default().0.iter().cloned().collect();

        let row_shares = row_shares.unwrap_or_default().0.iter().cloned().collect();

        let active_tab = active_tab.and_then(|id| Contents::try_from(&id.0.into()));

        let visible = visible.map_or(true, |v| v.0);

        let grid_columns = grid_columns.map(|v| v.0);

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

    /// Persist the entire [`ContainerBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`ContainerBlueprint`] was created with [`Self::from_egui_tiles_container`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        let timepoint = blueprint_timepoint_for_writes();

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

        let col_shares: ArrowBuffer<_> = col_shares.clone().into();
        let row_shares: ArrowBuffer<_> = row_shares.clone().into();

        let mut arch = crate::blueprint::archetypes::ContainerBlueprint::new(*container_kind)
            .with_display_name(display_name.clone())
            .with_contents(&contents)
            .with_col_shares(col_shares)
            .with_row_shares(row_shares)
            .with_visible(*visible);

        // TODO(jleibs): The need for this pattern is annoying. Should codegen
        // a version of this that can take an Option.
        if let Some(active_tab) = &active_tab {
            arch = arch.with_active_tab(&active_tab.as_entity_path());
        }

        if let Some(cols) = grid_columns {
            arch = arch.with_grid_columns(*cols);
        } else {
            // TODO(#3381): Archetypes should provide a convenience API for this
            ctx.save_empty_blueprint_component::<GridColumns>(&id.as_entity_path());
        }

        let mut deltas = vec![];

        if let Some(row) =
            DataRow::from_archetype(RowId::new(), timepoint.clone(), id.as_entity_path(), &arch)
                .warn_on_err_once("Failed to create Container blueprint.")
        {
            deltas.push(row);

            ctx.command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    ctx.store_context.blueprint.store_id().clone(),
                    deltas,
                ));
        }
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
                tile_to_contents.get(child_id).cloned().or_else(|| {
                    re_log::warn_once!("Missing child when building container.");
                    None
                })
            })
            .collect();

        match container {
            egui_tiles::Container::Tabs(tab) => {
                let active_tab = tab.active.and_then(|id| tile_to_contents.get(&id).cloned());

                Self {
                    id: container_id,
                    container_kind: egui_tiles::ContainerKind::Tabs,
                    display_name: format!("{:?}", egui_tiles::ContainerKind::Tabs),
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
                        display_name: format!("{kind:?}"),
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
                        display_name: format!("{kind:?}"),
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
                display_name: format!("{:?}", egui_tiles::ContainerKind::Grid),
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

    #[inline]
    pub fn set_visible(&self, ctx: &ViewerContext<'_>, visible: bool) {
        if visible != self.visible {
            let component = Visible(visible);
            ctx.save_blueprint_component(&self.entity_path(), &component);
        }
    }

    #[inline]
    pub fn set_grid_columns(&self, ctx: &ViewerContext<'_>, grid_columns: Option<u32>) {
        if grid_columns != self.grid_columns {
            if let Some(grid_columns) = grid_columns {
                let component = GridColumns(grid_columns);
                ctx.save_blueprint_component(&self.entity_path(), &component);
            } else {
                ctx.save_empty_blueprint_component::<GridColumns>(&self.entity_path());
            }
        }
    }

    /// Clears the blueprint component for this container.
    // TODO(jleibs): Should this be a recursive clear?
    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        let clear = Clear::recursive();
        ctx.save_blueprint_component(&self.entity_path(), &clear.is_recursive);
    }

    pub fn to_tile(&self) -> egui_tiles::Tile<SpaceViewId> {
        let children = self
            .contents
            .iter()
            .map(|item| item.as_tile_id())
            .collect::<Vec<_>>();

        let container = match self.container_kind {
            egui_tiles::ContainerKind::Tabs => {
                let mut tabs = egui_tiles::Tabs::new(children);
                tabs.active = self
                    .active_tab
                    .as_ref()
                    .map(|id| id.as_tile_id())
                    .or_else(|| tabs.children.first().copied());
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

                grid.col_shares = self.col_shares.clone();
                grid.row_shares = self.row_shares.clone();

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
