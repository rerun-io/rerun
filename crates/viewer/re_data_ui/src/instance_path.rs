use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_ui::{ContextExt as _, UiExt as _};
use re_viewer_context::{HoverHighlight, Item, UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            instance,
        } = self;

        let Some(components) = ctx
            .recording_store()
            .all_components_on_timeline(&query.timeline(), entity_path)
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
                    ui.ctx()
                        .error_text(format!("Unknown entity: {entity_path:?}")),
                );
            }
            return;
        };

        let components = crate::sorted_component_list_for_ui(&components);
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
                let is_static = db
                    .store()
                    .entity_has_static_component(entity_path, &component_name);
                let icon = if is_static {
                    &re_ui::icons::COMPONENT_STATIC
                } else {
                    &re_ui::icons::COMPONENT_TEMPORAL
                };
                let item = Item::ComponentPath(component_path);

                let mut list_item = ui.list_item().interactive(interactive);

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
                    let results = db.query_caches2().latest_at(
                        db.store(),
                        query,
                        entity_path,
                        [component_name],
                    );
                    let Some(unit) = results.components.get(&component_name) else {
                        continue; // no need to show components that are unset at this point in time
                    };

                    let content =
                        re_ui::list_item::PropertyContent::new(component_name.short_name())
                            .with_icon(icon)
                            .value_fn(|ui, _| {
                                if instance.is_all() {
                                    crate::EntityLatestAtResults {
                                        entity_path: entity_path.clone(),
                                        component_name,
                                        unit,
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
                                        component_name,
                                        unit,
                                        instance,
                                    );
                                }
                            });

                    list_item.show_flat(ui, content)
                };

                let response = response.on_hover_ui(|ui| {
                    component_name.data_ui_recording(ctx, ui, UiLayout::Tooltip);
                });

                if interactive {
                    ctx.select_hovered_on_click(&response, item);
                }
            }
        });
    }
}
