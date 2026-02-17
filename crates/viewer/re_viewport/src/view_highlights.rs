use egui::NumExt as _;
use nohash_hasher::IntMap;
use re_entity_db::InstancePath;
use re_log_types::EntityPathHash;
use re_renderer::OutlineMaskPreference;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::{
    DataResultInteractionAddress, HoverHighlight, Item, SelectionHighlight, ViewEntityHighlight,
    ViewHighlights, ViewId, ViewOutlineMasks,
};

/// Computes which things in a view should received highlighting.
///
/// This method makes decisions which entities & instances should which kind of highlighting
/// based on the entities in a view and the current selection/hover state.
pub fn highlights_for_view(
    ctx: &re_viewer_context::ViewerContext<'_>,
    view_id: ViewId,
) -> ViewHighlights {
    re_tracing::profile_function!();

    let mut highlighted_entity_paths = IntMap::<EntityPathHash, ViewEntityHighlight>::default();
    let mut outlines_masks = IntMap::<EntityPathHash, ViewOutlineMasks>::default();

    let mut selection_mask_index: u8 = 0;
    let mut next_selection_mask = || {
        // We don't expect to overflow u8, but if we do, don't use the "background mask".
        selection_mask_index = selection_mask_index.wrapping_add(1).at_least(1);
        OutlineMaskPreference::some(0, selection_mask_index)
    };

    let mut add_highlight_and_mask =
        |entity_hash: EntityPathHash,
         instance: InstancePath,
         visualizer_instruction: Option<VisualizerInstructionId>,
         highlight: SelectionHighlight| {
            highlighted_entity_paths
                .entry(entity_hash)
                .or_insert_with(|| ViewEntityHighlight::new(visualizer_instruction))
                .add_selection(&instance, highlight);
            outlines_masks
                .entry(entity_hash)
                .or_default()
                .add(&instance, next_selection_mask());
        };

    for current_selection in ctx.selection_state().selected_items().iter_items() {
        match current_selection {
            Item::AppId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::TableId(_)
            | Item::View(_)
            | Item::Container(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_) => {}

            Item::ComponentPath(component_path) => {
                let entity_hash = component_path.entity_path.hash();
                let instance = component_path.entity_path.clone().into();

                add_highlight_and_mask(
                    entity_hash,
                    instance,
                    None,
                    SelectionHighlight::SiblingSelection,
                );
            }

            Item::InstancePath(selected_instance) => {
                let entity_hash = selected_instance.entity_path.hash();
                let highlight = SelectionHighlight::SiblingSelection;
                add_highlight_and_mask(entity_hash, selected_instance.clone(), None, highlight);
            }

            Item::DataResult(DataResultInteractionAddress {
                view_id: selected_view_id,
                instance_path,
                visualizer,
            }) => {
                let entity_hash = instance_path.entity_path.hash();
                let highlight = if view_id == *selected_view_id {
                    SelectionHighlight::Selection
                } else {
                    SelectionHighlight::SiblingSelection
                };
                add_highlight_and_mask(entity_hash, instance_path.clone(), *visualizer, highlight);
            }
        }
    }

    let mut hover_mask_index: u8 = 0;
    let mut next_hover_mask = || {
        // We don't expect to overflow u8, but if we do, don't use the "background mask".
        hover_mask_index = hover_mask_index.wrapping_add(1).at_least(1);
        OutlineMaskPreference::some(hover_mask_index, 0)
    };

    for current_hover in ctx.selection_state().hovered_items().iter_items() {
        match current_hover {
            Item::AppId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::TableId(_)
            | Item::View(_)
            | Item::Container(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_) => {}

            Item::ComponentPath(component_path) => {
                let entity_hash = component_path.entity_path.hash();
                let instance = component_path.entity_path.clone().into();

                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_insert_with(|| ViewEntityHighlight::new(None))
                    .add_hover(&instance, HoverHighlight::Hovered);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(&instance, next_hover_mask());
            }

            Item::InstancePath(selected_instance) => {
                let entity_hash = selected_instance.entity_path.hash();
                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_insert_with(|| ViewEntityHighlight::new(None))
                    .add_hover(selected_instance, HoverHighlight::Hovered);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(selected_instance, next_hover_mask());
            }

            Item::DataResult(DataResultInteractionAddress {
                instance_path: selected_instance,
                view_id: _,
                visualizer,
            }) => {
                let entity_hash = selected_instance.entity_path.hash();
                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_insert_with(|| ViewEntityHighlight::new(*visualizer))
                    .add_hover(selected_instance, HoverHighlight::Hovered);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(selected_instance, next_hover_mask());
            }
        }
    }

    ViewHighlights {
        highlighted_entity_paths,
        outlines_masks,
    }
}
