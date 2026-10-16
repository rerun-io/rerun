//! Independent copies of configured container hierarchies.

use re_viewer_context::{ContainerId, Contents, ViewerContext};

use crate::{ContainerBlueprint, ViewBlueprint, ViewportBlueprint, ViewportCommand};

impl ContainerBlueprint {
    /// Copy this container's configuration, replacing each child with an independent copy.
    ///
    /// Unlike [`Clone::clone`], this creates a new identity and remaps the active tab.
    /// Returns `None` if any child cannot be copied.
    pub fn duplicate(
        &self,
        id: ContainerId,
        mut duplicate_child: impl FnMut(Contents) -> Option<Contents>,
    ) -> Option<Self> {
        let contents = self
            .contents
            .iter()
            .copied()
            .map(&mut duplicate_child)
            .collect::<Option<Vec<_>>>()?;
        let active_tab = self.active_tab.and_then(|active| {
            self.contents
                .iter()
                .position(|child| *child == active)
                .map(|index| contents[index])
        });

        Some(Self {
            id,
            contents,
            active_tab,
            ..self.clone()
        })
    }
}

impl ViewportBlueprint {
    /// Duplicate a container and all its descendants, inserting the copy next to the original.
    ///
    /// Cloning the root puts the original and the copy in a new tab container.
    /// The operation is deferred until the end of the frame, like other layout edits.
    pub fn duplicate_container(
        &self,
        container_id: &ContainerId,
        ctx: &ViewerContext<'_>,
    ) -> Option<ContainerId> {
        self.container(container_id)?;
        let new_id = ContainerId::random();
        self.enqueue_command(ViewportCommand::DuplicateContainer {
            source: *container_id,
            target: new_id,
        });
        self.mark_user_interaction(ctx);
        Some(new_id)
    }

    fn duplicate_container_tree(
        &self,
        source: ContainerId,
        target: ContainerId,
        ctx: &ViewerContext<'_>,
        containers: &mut Vec<ContainerBlueprint>,
        views: &mut Vec<ViewBlueprint>,
    ) -> Option<()> {
        let container = self.container(&source)?.duplicate(target, |child| {
            Some(match child {
                Contents::View(id) => {
                    let view = self
                        .view(&id)?
                        .duplicate(ctx.store_context, ctx.blueprint_query);
                    let new_id = view.id;
                    views.push(view);
                    Contents::View(new_id)
                }
                Contents::Container(id) => {
                    let new_id = ContainerId::random();
                    self.duplicate_container_tree(id, new_id, ctx, containers, views)?;
                    Contents::Container(new_id)
                }
            })
        })?;
        containers.push(container);
        Some(())
    }
}

pub(super) fn apply_duplicate_container(
    ctx: &ViewerContext<'_>,
    blueprint: &mut ViewportBlueprint,
    source: ContainerId,
    target: ContainerId,
) {
    let source_tile = Contents::Container(source).as_tile_id();
    if blueprint.tree.tiles.get(source_tile).is_none() {
        return;
    }

    // Construct the whole copy before making any changes, so a missing child cannot leave
    // a partly inserted hierarchy behind.
    let mut containers = Vec::new();
    let mut views = Vec::new();
    if blueprint
        .duplicate_container_tree(source, target, ctx, &mut containers, &mut views)
        .is_none()
    {
        re_log::warn_once!("Cannot clone a container with missing contents");
        return;
    }

    for view in views {
        let tile_id = Contents::View(view.id).as_tile_id();
        blueprint
            .tree
            .tiles
            .insert(tile_id, egui_tiles::Tile::Pane(view.id));
        blueprint.tree.set_visible(tile_id, view.visible);
        view.save_to_blueprint_store(ctx);
        blueprint.views.insert(view.id, view);
    }
    for container in containers {
        let tile_id = Contents::Container(container.id).as_tile_id();
        blueprint.tree.tiles.insert(tile_id, container.to_tile());
        blueprint.tree.set_visible(tile_id, container.visible);
        container.save_to_blueprint_store(ctx);
        blueprint.containers.insert(container.id, container);
    }

    let target_tile = Contents::Container(target).as_tile_id();
    if let Some(parent) = blueprint.tree.tiles.parent_of(source_tile) {
        if let Some(egui_tiles::Tile::Container(container)) = blueprint.tree.tiles.get_mut(parent) {
            let position = container
                .children()
                .position(|id| *id == source_tile)
                .unwrap_or(0);
            container.add_child(target_tile);
            blueprint
                .tree
                .move_tile_to_container(target_tile, parent, position + 1, true);
        }
    } else {
        blueprint.tree.root = Some(
            blueprint
                .tree
                .tiles
                .insert_tab_tile(vec![source_tile, target_tile]),
        );
    }

    // Reveal the copy when its parent is a tab container.
    blueprint.tree.make_active(|id, _| id == target_tile);
}
