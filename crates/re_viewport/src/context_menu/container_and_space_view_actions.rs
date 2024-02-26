use std::rc::Rc;

use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::{DataQueryBlueprint, SpaceViewBlueprint};
use re_viewer_context::{ContainerId, Item, SpaceViewClassIdentifier, SpaceViewId, ViewerContext};

use crate::context_menu::ContextMenuItem;
use crate::{Contents, ViewportBlueprint};

// ================================================================================================
// Space View/Container edit items
// ================================================================================================

/// Control the visibility of a container or space view
pub(super) struct ContentVisibilityToggle {
    contents: Rc<Vec<Contents>>,
    set_visible: bool,
}

impl ContentVisibilityToggle {
    pub(super) fn item(
        viewport_blueprint: &ViewportBlueprint,
        contents: Rc<Vec<Contents>>,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            set_visible: !contents
                .iter()
                .all(|item| viewport_blueprint.is_contents_visible(item)),
            contents,
        })
    }
}

impl ContextMenuItem for ContentVisibilityToggle {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        if self.set_visible {
            "Show".to_owned()
        } else {
            "Hide".to_owned()
        }
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        for content in &*self.contents {
            viewport_blueprint.set_content_visibility(ctx, content, self.set_visible);
        }
    }
}

/// Remove a container or space view
pub(super) struct ContentRemove {
    contents: Rc<Vec<Contents>>,
}

impl ContentRemove {
    pub(super) fn item(contents: Rc<Vec<Contents>>) -> Box<dyn ContextMenuItem> {
        Box::new(Self { contents })
    }
}

impl ContextMenuItem for ContentRemove {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        "Remove".to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        for content in &*self.contents {
            viewport_blueprint.mark_user_interaction(ctx);
            viewport_blueprint.remove_contents(*content);
        }
    }
}

// ================================================================================================
// Space view items
// ================================================================================================

/// Clone a space view
pub(super) struct CloneSpaceViewItem {
    space_view_id: SpaceViewId,
}

impl CloneSpaceViewItem {
    pub(super) fn item(space_view_id: SpaceViewId) -> Box<dyn ContextMenuItem> {
        Box::new(Self { space_view_id })
    }
}

impl ContextMenuItem for CloneSpaceViewItem {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        "Clone".to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        if let Some(new_space_view_id) =
            viewport_blueprint.duplicate_space_view(&self.space_view_id, ctx)
        {
            ctx.selection_state()
                .set_selection(Item::SpaceView(new_space_view_id));
            viewport_blueprint.mark_user_interaction(ctx);
        }
    }
}

// ================================================================================================
// Container items
// ================================================================================================

/// Add a container of a specific type
pub(super) struct AddContainer {
    target_container: ContainerId,
    container_kind: egui_tiles::ContainerKind,
}

impl AddContainer {
    pub(super) fn item(
        target_container: ContainerId,
        container_kind: egui_tiles::ContainerKind,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            target_container,
            container_kind,
        })
    }
}

impl ContextMenuItem for AddContainer {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        format!("{:?}", self.container_kind)
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.add_container(self.container_kind, Some(self.target_container));
        viewport_blueprint.mark_user_interaction(ctx);
    }
}

// ---

/// Add a space view of the specific class
pub(super) struct AddSpaceView {
    target_container: ContainerId,
    space_view_class: SpaceViewClassIdentifier,
}

impl AddSpaceView {
    pub(super) fn item(
        target_container: ContainerId,
        space_view_class: SpaceViewClassIdentifier,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            target_container,
            space_view_class,
        })
    }
}

impl ContextMenuItem for AddSpaceView {
    fn label(&self, ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        ctx.space_view_class_registry
            .get_class_or_log_error(&self.space_view_class)
            .display_name()
            .to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        let space_view = SpaceViewBlueprint::new(
            self.space_view_class,
            &EntityPath::root(),
            DataQueryBlueprint::new(self.space_view_class, EntityPathFilter::default()),
        );

        viewport_blueprint.add_space_views(
            std::iter::once(space_view),
            ctx,
            Some(self.target_container),
            None,
        );
        viewport_blueprint.mark_user_interaction(ctx);
    }
}

// ---

/// Move the selected contents to a newly created container of the given kind
pub(super) struct MoveContentsToNewContainer {
    parent_container: ContainerId,
    position_in_parent: usize,
    container_kind: egui_tiles::ContainerKind,
    contents: Rc<Vec<Contents>>,
}

impl MoveContentsToNewContainer {
    pub(super) fn item(
        parent_container: ContainerId,
        position_in_parent: usize,
        container_kind: egui_tiles::ContainerKind,
        contents: Rc<Vec<Contents>>,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            parent_container,
            position_in_parent,
            container_kind,
            contents,
        })
    }
}

impl ContextMenuItem for MoveContentsToNewContainer {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        format!("{:?}", self.container_kind)
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.move_contents_to_new_container(
            (*self.contents).clone(),
            self.container_kind,
            self.parent_container,
            self.position_in_parent,
        );

        viewport_blueprint.mark_user_interaction(ctx);
    }
}
