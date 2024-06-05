use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_viewer_context::{HoverHighlight, Item, UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            instance,
        } = self;

        let Some(components) = ctx
            .recording_store()
            .all_components(&query.timeline(), entity_path)
        else {
            if ctx.recording().is_known_entity(entity_path) {
                // This is fine - e.g. we're looking at `/world` and the user has only logged to `/world/car`.
                ui_layout.label(
                    ui,
                    format!(
                        "No components logged on timeline {:?}",
                        query.timeline().name()
                    ),
                );
            } else {
                ui_layout.label(
                    ui,
                    ctx.re_ui
                        .error_text(format!("Unknown entity: {entity_path:?}")),
                );
            }
            return;
        };

        let components = crate::component_list_for_ui(&components);
        let indicator_count = components
            .iter()
            .filter(|c| c.is_indicator_component())
            .count();

        if ui_layout == UiLayout::List {
            ui_layout.label(
                ui,
                format!(
                    "{} component{} (including {} indicator component{})",
                    components.len(),
                    if components.len() > 1 { "s" } else { "" },
                    indicator_count,
                    if indicator_count > 1 { "s" } else { "" }
                ),
            );
            return;
        }

        let show_indicator_comps = match ui_layout {
            UiLayout::Tooltip => {
                // Skip indicator components in hover ui (unless there are no other
                // types of components).
                indicator_count == components.len()
            }
            UiLayout::SelectionPanelLimitHeight | UiLayout::SelectionPanelFull => true,
            UiLayout::List => false, // unreachable
        };

        let interactive = ui_layout != UiLayout::Tooltip;

        re_ui::list_item::list_item_scope(ui, "component list", |ui| {
            for component_name in components {
                if !show_indicator_comps && component_name.is_indicator_component() {
                    continue;
                }

                let component_path = ComponentPath::new(entity_path.clone(), component_name);
                let is_static = db.is_component_static(&component_path).unwrap_or_default();
                let icon = if is_static {
                    &re_ui::icons::COMPONENT_STATIC
                } else {
                    &re_ui::icons::COMPONENT_TEMPORAL
                };
                let item = Item::ComponentPath(component_path);

                let mut list_item = ctx.re_ui.list_item().interactive(interactive);

                if interactive {
                    let is_hovered = ctx.selection_state().highlight_for_ui_element(&item)
                        == HoverHighlight::Hovered;
                    list_item = list_item.force_hovered(is_hovered);
                }

                let response = if component_name.is_indicator_component() {
                    list_item.show_flat(
                        ui,
                        re_ui::list_item::LabelContent::new(component_name.short_name())
                            .with_icon(icon),
                    )
                } else {
                    let results = db.query_caches().latest_at(
                        db.store(),
                        query,
                        entity_path,
                        [component_name],
                    );
                    let Some(results) = results.components.get(&component_name) else {
                        continue; // no need to show components that are unset at this point in time
                    };

                    let mut content =
                        re_ui::list_item::PropertyContent::new(component_name.short_name())
                            .with_icon(icon)
                            .value_fn(|_, ui, _| {
                                if instance.is_all() {
                                    crate::EntityLatestAtResults {
                                        entity_path: entity_path.clone(),
                                        component_name,
                                        results: std::sync::Arc::clone(results),
                                    }
                                    .data_ui(
                                        ctx,
                                        ui,
                                        UiLayout::List,
                                        query,
                                        db,
                                    );
                                } else {
                                    ctx.component_ui_registry.ui(
                                        ctx,
                                        ui,
                                        UiLayout::List,
                                        query,
                                        db,
                                        entity_path,
                                        results,
                                        instance,
                                    );
                                }
                            });

                    // avoid the list item to max the tooltip width
                    if ui_layout == UiLayout::Tooltip {
                        content = content.exact_width(true);
                    }

                    list_item.show_flat(ui, content)
                };

                if interactive {
                    ctx.select_hovered_on_click(&response, item);
                }
            }
        });
    }
}
