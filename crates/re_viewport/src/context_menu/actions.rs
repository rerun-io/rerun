use egui::{Response, Ui};
use itertools::Itertools;
use nohash_hasher::IntSet;

use re_entity_db::InstancePath;
use re_log_types::{EntityPath, EntityPathFilter, EntityPathRule, RuleEffect};
use re_space_view::{determine_visualizable_entities, DataQueryBlueprint, SpaceViewBlueprint};
use re_viewer_context::{ContainerId, Item, SpaceViewClassIdentifier, SpaceViewId};

use super::{ContextMenuAction, ContextMenuContext};
use crate::Contents;

pub(super) struct ShowAction;

impl ContextMenuAction for ShowAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::SpaceView(space_view_id) => !ctx
                .viewport_blueprint
                .is_contents_visible(&Contents::SpaceView(*space_view_id)),
            Item::Container(container_id) => {
                !ctx.viewport_blueprint
                    .is_contents_visible(&Contents::Container(*container_id))
                    && ctx.viewport_blueprint.root_container != Some(*container_id)
            }
            Item::DataResult(space_view_id, instance_path) => {
                data_result_visible(ctx, space_view_id, instance_path).is_some_and(|vis| !vis)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Show All".to_owned()
        } else {
            "Show".to_owned()
        }
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::Container(*container_id),
            true,
        );
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::SpaceView(*space_view_id),
            true,
        );
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_visible(ctx, space_view_id, instance_path, true);
    }
}

pub(super) struct HideAction;

impl ContextMenuAction for HideAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::SpaceView(space_view_id) => ctx
                .viewport_blueprint
                .is_contents_visible(&Contents::SpaceView(*space_view_id)),
            Item::Container(container_id) => {
                ctx.viewport_blueprint
                    .is_contents_visible(&Contents::Container(*container_id))
                    && ctx.viewport_blueprint.root_container != Some(*container_id)
            }
            Item::DataResult(space_view_id, instance_path) => {
                data_result_visible(ctx, space_view_id, instance_path).unwrap_or(false)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Hide All".to_owned()
        } else {
            "Hide".to_owned()
        }
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::Container(*container_id),
            false,
        );
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        ctx.viewport_blueprint.set_content_visibility(
            ctx.viewer_context,
            &Contents::SpaceView(*space_view_id),
            false,
        );
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_visible(ctx, space_view_id, instance_path, false);
    }
}

fn data_result_visible(
    ctx: &ContextMenuContext<'_>,
    space_view_id: &SpaceViewId,
    instance_path: &InstancePath,
) -> Option<bool> {
    instance_path
        .is_splat()
        .then(|| {
            ctx.viewport_blueprint
                .space_view(space_view_id)
                .and_then(|space_view| {
                    let query_result = ctx
                        .viewer_context
                        .lookup_query_result(space_view.query_id());
                    query_result
                        .tree
                        .lookup_result_by_path(&instance_path.entity_path)
                        .map(|data_result| {
                            data_result
                                .recursive_properties()
                                .map_or(true, |prop| prop.visible)
                        })
                })
        })
        .flatten()
}

fn set_data_result_visible(
    ctx: &ContextMenuContext<'_>,
    space_view_id: &SpaceViewId,
    instance_path: &InstancePath,
    visible: bool,
) {
    if let Some(space_view) = ctx.viewport_blueprint.space_view(space_view_id) {
        let query_result = ctx
            .viewer_context
            .lookup_query_result(space_view.query_id());
        if let Some(data_result) = query_result
            .tree
            .lookup_result_by_path(&instance_path.entity_path)
        {
            let mut recursive_properties = data_result
                .recursive_properties()
                .cloned()
                .unwrap_or_default();
            recursive_properties.visible = visible;

            data_result.save_recursive_override(ctx.viewer_context, Some(recursive_properties));
        }
    }
}

// ---

/// Remove a container, space view, or data result.
pub(super) struct RemoveAction;

impl ContextMenuAction for RemoveAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::SpaceView(_) => true,
            Item::Container(container_id) => {
                ctx.viewport_blueprint.root_container != Some(*container_id)
            }
            Item::DataResult(_, instance_path) => instance_path.is_splat(),
            _ => false,
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Remove".to_owned()
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
        ctx.viewport_blueprint
            .remove_contents(Contents::Container(*container_id));
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
        ctx.viewport_blueprint
            .remove_contents(Contents::SpaceView(*space_view_id));
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        space_view_id: &SpaceViewId,
        instance_path: &InstancePath,
    ) {
        if let Some(space_view) = ctx.viewport_blueprint.space_view(space_view_id) {
            space_view.add_entity_exclusion(
                ctx.viewer_context,
                EntityPathRule::including_subtree(instance_path.entity_path.clone()),
            );
        }
    }
}

// ---

/// Clone a single space view
pub(super) struct CloneSpaceViewAction;

impl ContextMenuAction for CloneSpaceViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::SpaceView(_))
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Clone".to_owned()
    }

    fn process_space_view(&self, ctx: &ContextMenuContext<'_>, space_view_id: &SpaceViewId) {
        if let Some(new_space_view_id) = ctx
            .viewport_blueprint
            .duplicate_space_view(space_view_id, ctx.viewer_context)
        {
            ctx.viewer_context
                .selection_state()
                .set_selection(Item::SpaceView(new_space_view_id));
            ctx.viewport_blueprint
                .mark_user_interaction(ctx.viewer_context);
        }
    }
}

// ---

/// Add a container of a specific type
pub(super) struct AddContainerAction(pub egui_tiles::ContainerKind);

impl ContextMenuAction for AddContainerAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        if let Some(Item::Container(container_id)) = ctx.selection.single_item() {
            if let Some(container) = ctx.viewport_blueprint.container(container_id) {
                // same-kind linear containers cannot be nested
                (container.container_kind != egui_tiles::ContainerKind::Vertical
                    && container.container_kind != egui_tiles::ContainerKind::Horizontal)
                    || container.container_kind != self.0
            } else {
                // unknown container
                false
            }
        } else {
            false
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        format!("{:?}", self.0)
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        ctx.viewport_blueprint
            .add_container(self.0, Some(*container_id));
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}

// ---

/// Add a space view of the specific class
pub(super) struct AddSpaceViewAction(pub SpaceViewClassIdentifier);

impl ContextMenuAction for AddSpaceViewAction {
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::Container(_))
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        ctx.viewer_context
            .space_view_class_registry
            .get_class_or_log_error(&self.0)
            .display_name()
            .to_owned()
    }

    fn process_container(&self, ctx: &ContextMenuContext<'_>, container_id: &ContainerId) {
        let space_view = SpaceViewBlueprint::new(
            self.0,
            &EntityPath::root(),
            DataQueryBlueprint::new(self.0, EntityPathFilter::default()),
        );

        ctx.viewport_blueprint.add_space_views(
            std::iter::once(space_view),
            ctx.viewer_context,
            Some(*container_id),
            None,
        );
        ctx.viewport_blueprint
            .mark_user_interaction(ctx.viewer_context);
    }
}

// ---

/// Move the selected contents to a newly created container of the given kind
pub(super) struct MoveContentsToNewContainerAction(pub egui_tiles::ContainerKind);

impl ContextMenuAction for MoveContentsToNewContainerAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        if let Some((parent_container, _)) = ctx.clicked_item_parent_and_position() {
            if (parent_container.container_kind == egui_tiles::ContainerKind::Vertical
                || parent_container.container_kind == egui_tiles::ContainerKind::Horizontal)
                && parent_container.container_kind == self.0
            {
                return false;
            }
        }

        ctx.selection.iter().all(|(item, _)| match item {
            Item::SpaceView(_) => true,
            Item::Container(container_id) => {
                ctx.viewport_blueprint.root_container != Some(*container_id)
            }
            _ => false,
        })
    }

    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::SpaceView(_) => true,
            Item::Container(container_id) => {
                ctx.viewport_blueprint.root_container != Some(*container_id)
            }
            _ => false,
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        format!("{:?}", self.0)
    }

    fn process_selection(&self, ctx: &ContextMenuContext<'_>) {
        if let Some(root_container_id) = ctx.viewport_blueprint.root_container {
            let (target_container_id, target_position) = ctx
                .clicked_item_parent_id_and_position()
                .unwrap_or((root_container_id, 0));

            let contents = ctx
                .selection
                .iter()
                .filter_map(|(item, _)| item.try_into().ok())
                .collect();

            ctx.viewport_blueprint.move_contents_to_new_container(
                contents,
                self.0,
                target_container_id,
                target_position,
            );

            ctx.viewport_blueprint
                .mark_user_interaction(ctx.viewer_context);
        }
    }
}

// ---

/// Create a new space view containing the selected entities.
///
/// The space view is created next to the clicked item's parent view (if a data result was clicked).
pub(super) struct AddEntitiesToNewSpaceViewAction;

impl ContextMenuAction for AddEntitiesToNewSpaceViewAction {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        matches!(item, Item::DataResult(_, _) | Item::InstancePath(_))
    }

    fn ui(&self, ctx: &ContextMenuContext<'_>, ui: &mut Ui) -> Response {
        let space_view_class_registry = ctx.viewer_context.space_view_class_registry;

        let recommended_space_view_classes = recommended_space_views_for_selection(ctx);
        let other_space_view_classes: IntSet<_> = space_view_class_registry
            .iter_registry()
            .map(|entry| entry.class.identifier())
            .collect::<IntSet<SpaceViewClassIdentifier>>()
            .difference(&recommended_space_view_classes)
            .cloned()
            .collect();

        ui.menu_button("Add to new space view", |ui| {
            let buttons_for_space_view_classes =
                |ui: &mut egui::Ui, space_view_classes: &IntSet<SpaceViewClassIdentifier>| {
                    for (identifier, display_name) in space_view_classes
                        .iter()
                        .map(|identifier| {
                            (
                                identifier,
                                space_view_class_registry
                                    .get_class_or_log_error(&identifier)
                                    .display_name(),
                            )
                        })
                        .sorted_by_key(|(_, display_name)| display_name.to_owned())
                    {
                        if ui.button(display_name).clicked() {
                            create_space_view_with_entities(ctx, *identifier);
                        }
                    }
                };

            ui.label(egui::WidgetText::from("Recommended:").italics());
            buttons_for_space_view_classes(ui, &recommended_space_view_classes);
            ui.label(egui::WidgetText::from("Others:").italics());
            buttons_for_space_view_classes(ui, &other_space_view_classes);
        })
        .response
    }
}

/// Builds a list of compatible space views for the provided selection.
fn recommended_space_views_for_selection(
    ctx: &ContextMenuContext<'_>,
) -> IntSet<SpaceViewClassIdentifier> {
    let entities_of_interest = ctx
        .selection
        .iter()
        .filter_map(|(item, _)| item.entity_path())
        .collect::<Vec<_>>();

    let mut output: IntSet<SpaceViewClassIdentifier> = IntSet::default();

    let space_view_class_registry = ctx.viewer_context.space_view_class_registry;
    let entity_db = ctx.viewer_context.entity_db;
    let applicable_entities_per_visualizer =
        space_view_class_registry.applicable_entities_for_visualizer_systems(entity_db.store_id());

    for entry in space_view_class_registry.iter_registry() {
        let visualizable_entities = determine_visualizable_entities(
            &applicable_entities_per_visualizer,
            entity_db,
            &space_view_class_registry.new_visualizer_collection(entry.class.identifier()),
            &*entry.class,
            &EntityPath::root(),
        );

        let covered = entities_of_interest.iter().all(|entity| {
            visualizable_entities
                .0
                .iter()
                .any(|(_, entities)| entities.0.contains(*entity))
        });

        if covered {
            output.insert(entry.class.identifier());
        }
    }

    output
}

/// Creates a space view of the give class, with root set as origin, and a filter set to include all
/// selected entities.
fn create_space_view_with_entities(
    ctx: &ContextMenuContext<'_>,
    identifier: SpaceViewClassIdentifier,
) {
    let origin = EntityPath::root();

    let mut filter = EntityPathFilter::default();
    ctx.selection
        .iter()
        .filter_map(|(item, _)| item.entity_path())
        .for_each(|path| {
            filter.add_rule(
                RuleEffect::Include,
                EntityPathRule::including_subtree(path.clone()),
            );
        });

    let target_container_id = ctx.clicked_item_parent_id_and_position().map(|(id, _)| id);

    let space_view = SpaceViewBlueprint::new(
        identifier,
        &origin,
        DataQueryBlueprint::new(identifier, filter),
    );

    ctx.viewport_blueprint.add_space_views(
        std::iter::once(space_view),
        ctx.viewer_context,
        target_container_id,
        None,
    );
    ctx.viewport_blueprint
        .mark_user_interaction(ctx.viewer_context);
}
