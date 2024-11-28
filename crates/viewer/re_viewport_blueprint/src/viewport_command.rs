use re_viewer_context::{ContainerId, Contents, SpaceViewId};

use crate::SpaceViewBlueprint;

/// Mutation actions to perform on the viewport tree at the end of the frame.
///
/// These are deferred so that we have an immutable viewport during the duration of the frame.
#[derive(Clone, Debug)]
pub enum ViewportCommand {
    /// Set the whole viewport tree.
    SetTree(egui_tiles::Tree<SpaceViewId>),

    /// Add a new space view to the provided container (or the root if `None`).
    AddSpaceView {
        space_view: SpaceViewBlueprint,
        parent_container: Option<ContainerId>,
        position_in_parent: Option<usize>,
    },

    /// Add a new container of the provided kind to the provided container (or the root if `None`).
    AddContainer {
        container_kind: egui_tiles::ContainerKind,
        parent_container: Option<ContainerId>,
    },

    /// Change the kind of a container.
    SetContainerKind(ContainerId, egui_tiles::ContainerKind),

    /// Ensure the tab for the provided space view is focused (see [`egui_tiles::Tree::make_active`]).
    FocusTab(SpaceViewId),

    /// Remove a container (recursively) or a space view
    RemoveContents(Contents),

    /// Simplify the container with the provided options
    SimplifyContainer(ContainerId, egui_tiles::SimplificationOptions),

    /// Make all column and row shares the same for this container
    MakeAllChildrenSameSize(ContainerId),

    /// Move some contents to a different container
    MoveContents {
        contents_to_move: Contents,
        target_container: ContainerId,
        target_position_in_container: usize,
    },

    /// Move one or more [`Contents`] to a newly created container
    MoveContentsToNewContainer {
        contents_to_move: Vec<Contents>,
        new_container_kind: egui_tiles::ContainerKind,
        target_container: ContainerId,
        target_position_in_container: usize,
    },
}
