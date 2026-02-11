use std::ops::Range;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_data_ui::item_ui;
use re_entity_db::{EntityPath, EntityTree, InstancePath};
use re_log_types::{ResolvedEntityPathFilter, ResolvedEntityPathRule};
use re_ui::filter_widget::{FilterMatcher, FilterState, PathRanges, format_matching_text};
use re_ui::{UiExt as _, list_item};
use re_viewer_context::{DataQueryResult, ViewId, ViewerContext};
use re_viewport_blueprint::{
    CanAddToView, EntityAddInfo, ViewBlueprint, ViewportBlueprint, create_entity_add_info,
};
use smallvec::SmallVec;

/// Window for adding/removing entities from a view.
///
/// Delegates to [`re_ui::modal::ModalHandler`]
#[derive(Default)]
pub(crate) struct ViewEntityPicker {
    view_id: Option<ViewId>,
    modal_handler: re_ui::modal::ModalHandler,
    filter_state: FilterState,
}

impl ViewEntityPicker {
    pub fn open(&mut self, view_id: ViewId) {
        self.view_id = Some(view_id);
        self.filter_state = FilterState::default();
        self.modal_handler.open();
    }

    pub fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
    ) {
        self.modal_handler.ui(
            egui_ctx,
            || {
                re_ui::modal::ModalWrapper::new("Add/remove Entities")
                    .min_height(f32::min(160.0, egui_ctx.content_rect().height() * 0.8))
                    .full_span_content(true)
                    // we set the scroll area ourselves
                    .set_side_margin(false)
                    .scrollable([false, false])
            },
            |ui| {
                // 80%, never more than 500px
                ui.set_max_height(f32::min(ui.ctx().content_rect().height() * 0.8, 500.0));
                let Some(view_id) = &self.view_id else {
                    ui.close();
                    return;
                };

                let Some(view) = viewport_blueprint.view(view_id) else {
                    ui.close();
                    return;
                };

                ui.add_space(5.0);
                ui.panel_content(|ui| {
                    self.filter_state.search_field_ui(ui, "Search for entity…");
                });
                ui.add_space(5.0);

                egui::ScrollArea::new([false, true]).show(ui, |ui| {
                    ui.panel_content(|ui| {
                        let matcher = self.filter_state.filter();
                        add_entities_ui(ctx, ui, view, &matcher, self.filter_state.session_id());
                    });
                });
            },
        );
    }
}

fn add_entities_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view: &ViewBlueprint,
    filter_matcher: &FilterMatcher,
    filter_session_id: Option<egui::Id>,
) {
    re_tracing::profile_function!();

    let tree = &ctx.recording().tree();
    let query_result = ctx.lookup_query_result(view.id);
    let entity_path_filter = view.contents.entity_path_filter();
    let entities_add_info = create_entity_add_info(ctx, tree, view, query_result);

    let mut hierarchy = Default::default();
    let mut hierarchy_highlights = Default::default();
    let entity_data = EntityPickerEntryData::from_entity_tree_and_filter(
        &view.space_origin,
        tree,
        filter_matcher,
        &mut hierarchy,
        &mut hierarchy_highlights,
    );

    if let Some(entity_data) = entity_data {
        list_item::list_item_scope(ui, "view_entity_picker", |ui| {
            add_entities_tree_ui(
                ctx,
                ui,
                &entity_data,
                view,
                query_result,
                entity_path_filter,
                &entities_add_info,
                filter_session_id,
            );
        });
    } else {
        ui.label("No entities match the filter.");
    }
}

#[expect(clippy::too_many_arguments)]
fn add_entities_tree_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_data: &EntityPickerEntryData,
    view: &ViewBlueprint,
    query_result: &DataQueryResult,
    entity_path_filter: &ResolvedEntityPathFilter,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
    filter_session_id: Option<egui::Id>,
) {
    let item_content = list_item::CustomContent::new(|ui, _| {
        add_entities_line_ui(
            ctx,
            ui,
            entity_data,
            view,
            query_result,
            entity_path_filter,
            entities_add_info,
        );
    });

    let list_item = ui.list_item().interactive(false);
    if entity_data.is_leaf() {
        list_item.show_hierarchical(ui, item_content);
    } else {
        let level = entity_data.entity_path.len();
        let default_open = view.space_origin.is_descendant_of(&entity_data.entity_path)
            || entity_data.children.len() <= 3
            || level < 2
            || filter_session_id.is_some();

        list_item.show_hierarchical_with_children(
            ui,
            ui.id()
                .with(&entity_data.entity_path)
                .with(filter_session_id),
            default_open,
            item_content,
            |ui| {
                for children in &entity_data.children {
                    add_entities_tree_ui(
                        ctx,
                        ui,
                        children,
                        view,
                        query_result,
                        entity_path_filter,
                        entities_add_info,
                        filter_session_id,
                    );
                }
            },
        );
    }
}

fn add_entities_line_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_data: &EntityPickerEntryData,
    view: &ViewBlueprint,
    query_result: &DataQueryResult,
    entity_path_filter: &ResolvedEntityPathFilter,
    entities_add_info: &IntMap<EntityPath, EntityAddInfo>,
) {
    re_tracing::profile_function!();

    let query = ctx.current_query();
    let entity_path = &entity_data.entity_path;
    let name = &entity_data.label;

    let Some(add_info) = entities_add_info.get(entity_path) else {
        // No add info implies that there can't be an add line ui, shouldn't get here.
        debug_assert!(false, "No add info for entity path: {entity_path:?}");
        return;
    };

    let is_explicitly_excluded = entity_path_filter.is_explicitly_excluded(entity_path);
    let is_explicitly_included = entity_path_filter.is_explicitly_included(entity_path);
    let is_included = entity_path_filter.matches(entity_path);

    ui.add_enabled_ui(add_info.can_add_self_or_descendant.is_compatible(), |ui| {
        let mut widget_text = format_matching_text(
            ctx.egui_ctx(),
            name,
            entity_data.highlight_sections.iter().cloned(),
            None,
        );

        if is_explicitly_excluded {
            // TODO(jleibs): Better design-language for excluded.
            widget_text = widget_text.italics();
        } else if entity_path == &view.space_origin {
            widget_text = widget_text.strong();
        }

        let response = item_ui::instance_path_button_to(
            ctx,
            &query,
            ctx.recording(),
            ui,
            Some(view.id),
            &InstancePath::entity_all(entity_path.clone()),
            widget_text,
        );
        if query_result.result_for_entity(entity_path).is_some() {
            response.highlight();
        }
    });

    //TODO(ab): use `CustomContent` support for action button to implement this.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if entity_path_filter.contains_rule_for_exactly(entity_path) {
            if ResolvedEntityPathFilter::properties().matches(entity_path) {
                let enabled = add_info.can_add_self_or_descendant.is_compatible();

                ui.add_enabled_ui(enabled, |ui| {
                    let response = ui.small_icon_button(&re_ui::icons::ADD, "Include entity");

                    if response.clicked() {
                        view.contents.remove_filter_rule_for(ctx, entity_path);
                        view.contents.raw_add_entity_inclusion(
                            ctx,
                            ResolvedEntityPathRule::including_subtree(entity_path),
                        );
                    }

                    if enabled {
                        if add_info.can_add.is_compatible_and_missing() {
                            response.on_hover_text(
                                "Include this entity and all its descendants in the view",
                            );
                        } else {
                            response.on_hover_text("Add descendants of this entity to the view");
                        }
                    } else if let CanAddToView::No { reason } = &add_info.can_add {
                        response.on_disabled_hover_text(reason);
                    }
                });
            } else {
                // Reset-button
                // Shows when an entity is explicitly excluded or included
                let response = ui.small_icon_button(&re_ui::icons::RESET, "Remove this rule");

                if response.clicked() {
                    view.contents.remove_filter_rule_for(ctx, entity_path);
                }

                if is_explicitly_excluded {
                    response.on_hover_text("Stop excluding this entity path.");
                } else if is_explicitly_included {
                    response.on_hover_text("Stop including this entity path.");
                }
            }
        } else if is_included {
            // Remove-button
            // Shows when an entity is already included (but not explicitly)
            let response = ui.small_icon_button(&re_ui::icons::REMOVE, "Exclude entity");

            if response.clicked() {
                view.contents.raw_add_entity_exclusion(
                    ctx,
                    ResolvedEntityPathRule::including_subtree(entity_path),
                );
            }

            response.on_hover_text("Exclude this entity and all its descendants from the view");
        } else {
            // Add-button:
            // Shows when an entity is not included
            // Only enabled if the entity is compatible.
            let enabled = add_info.can_add_self_or_descendant.is_compatible();

            ui.add_enabled_ui(enabled, |ui| {
                let response = ui.small_icon_button(&re_ui::icons::ADD, "Include entity");

                if response.clicked() {
                    view.contents.raw_add_entity_inclusion(
                        ctx,
                        ResolvedEntityPathRule::including_subtree(entity_path),
                    );
                }

                if enabled {
                    if add_info.can_add.is_compatible_and_missing() {
                        response.on_hover_text(
                            "Include this entity and all its descendants in the view",
                        );
                    } else {
                        response.on_hover_text("Add descendants of this entity to the view");
                    }
                } else if let CanAddToView::No { reason } = &add_info.can_add {
                    response.on_disabled_hover_text(reason);
                }
            });
        }
    });
}

// ---

#[derive(Debug)]
struct EntityPickerEntryData {
    pub entity_path: EntityPath,
    pub label: String,
    pub highlight_sections: SmallVec<[Range<usize>; 1]>,
    pub children: Vec<Self>,
}

impl EntityPickerEntryData {
    fn from_entity_tree_and_filter(
        view_origin: &EntityPath,
        entity_tree: &EntityTree,
        filter_matcher: &FilterMatcher,
        hierarchy: &mut Vec<String>,
        hierarchy_highlights: &mut PathRanges,
    ) -> Option<Self> {
        let entity_part_ui_string = entity_tree
            .path
            .last()
            .map(|entity_part| entity_part.ui_string());
        let mut label = entity_part_ui_string
            .clone()
            .unwrap_or_else(|| "/".to_owned());

        let must_pop = if let Some(part) = &entity_part_ui_string {
            hierarchy.push(part.clone());
            true
        } else {
            false
        };

        //
        // Gather some info about the current node…
        //

        /// Temporary structure to hold local information.
        struct NodeInfo {
            is_leaf: bool,
            is_this_a_match: bool,
            children: Vec<EntityPickerEntryData>,
        }

        let node_info = if entity_tree.children.is_empty() {
            // Key insight: we only ever need to match the hierarchy from the leaf nodes.
            // Non-leaf nodes know they are a match if any child remains after walking their
            // subtree.

            let highlights = filter_matcher.match_path(hierarchy.iter().map(String::as_str));

            let is_this_a_match = if let Some(highlights) = highlights {
                hierarchy_highlights.merge(highlights);
                true
            } else {
                false
            };

            NodeInfo {
                is_leaf: true,
                is_this_a_match,
                children: vec![],
            }
        } else {
            let mut children = entity_tree
                .children
                .values()
                .filter_map(|sub_tree| {
                    Self::from_entity_tree_and_filter(
                        view_origin,
                        sub_tree,
                        filter_matcher,
                        hierarchy,
                        hierarchy_highlights,
                    )
                })
                .collect_vec();

            // Always have descendent of the view origin first.
            children.sort_by_key(|child| {
                let put_first = child.entity_path.starts_with(view_origin);
                !put_first
            });

            let is_this_a_match = !children.is_empty();

            NodeInfo {
                is_leaf: false,
                is_this_a_match,
                children,
            }
        };

        //
        // …then handle the node accordingly.
        //

        let result = node_info.is_this_a_match.then(|| {
            let highlight_sections = hierarchy_highlights
                .remove(hierarchy.len().saturating_sub(1))
                .map(Iterator::collect)
                .unwrap_or_default();

            if !node_info.is_leaf && !entity_tree.path.is_root() {
                // Indicate that we have children
                label.push('/');
            }
            Self {
                entity_path: entity_tree.path.clone(),
                label,
                highlight_sections,
                children: node_info.children,
            }
        });

        if must_pop {
            hierarchy_highlights.remove(hierarchy.len().saturating_sub(1));
            hierarchy.pop();
        }

        result
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}
