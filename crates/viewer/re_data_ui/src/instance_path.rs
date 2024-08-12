use nohash_hasher::IntMap;

use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_types::{archetypes, components, image::ImageKind, Archetype, ComponentName, Loggable};
use re_ui::{ContextExt as _, UiExt as _};
use re_viewer_context::{
    gpu_bridge::image_to_gpu, HoverHighlight, ImageInfo, ImageStatsCache, Item, UiLayout,
    ViewerContext,
};

use crate::image::texture_preview_ui;

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

        let components = latest_at(db, query, entity_path, &components);

        if ui_layout.is_single_line() {
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
        } else {
            component_list_ui(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                instance,
                &components,
            );
        }

        if instance.is_all() {
            let component_map = components.into_iter().collect();
            preview_if_image_ui(ctx, ui, ui_layout, query, entity_path, &component_map);
        }
    }
}

fn latest_at(
    db: &re_entity_db::EntityDb,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    components: &[ComponentName],
) -> Vec<(ComponentName, UnitChunkShared)> {
    let components: Vec<(ComponentName, UnitChunkShared)> = components
        .iter()
        .filter_map(|&component_name| {
            let mut results =
                db.query_caches()
                    .latest_at(db.store(), query, entity_path, [component_name]);

            // We ignore components that are unset at this point in time
            results
                .components
                .remove(&component_name)
                .map(|unit| (component_name, unit))
        })
        .collect();
    components
}

#[allow(clippy::too_many_arguments)]
fn component_list_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    db: &re_entity_db::EntityDb,
    entity_path: &re_log_types::EntityPath,
    instance: &re_log_types::Instance,
    components: &[(ComponentName, UnitChunkShared)],
) {
    let indicator_count = components
        .iter()
        .filter(|(c, _)| c.is_indicator_component())
        .count();

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
        for (component_name, unit) in components {
            let component_name = *component_name;
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
                let content = re_ui::list_item::PropertyContent::new(component_name.short_name())
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

/// If this entity is an image, show it together with buttons to download and copy the image.
fn preview_if_image_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    component_map: &IntMap<ComponentName, UnitChunkShared>,
) -> Option<()> {
    let image_buffer = component_map.get(&components::ImageBuffer::name())?;
    let buffer_row_id = image_buffer.row_id()?;
    let image_buffer = image_buffer
        .component_mono::<components::ImageBuffer>()?
        .ok()?;

    let image_format = component_map
        .get(&components::ImageFormat::name())?
        .component_mono::<components::ImageFormat>()?
        .ok()?;

    let kind = if component_map.contains_key(&archetypes::DepthImage::indicator().name()) {
        ImageKind::Depth
    } else if component_map.contains_key(&archetypes::SegmentationImage::indicator().name()) {
        ImageKind::Segmentation
    } else {
        ImageKind::Color
    };

    let colormap = component_map
        .get(&components::Colormap::name())
        .and_then(|colormap| {
            colormap
                .component_mono::<components::Colormap>()
                .transpose()
                .ok()
                .flatten()
        });

    let image = ImageInfo {
        buffer_row_id,
        buffer: image_buffer.0,
        format: image_format.0,
        kind,
        colormap,
    };

    image_preview_ui(ctx, ui, ui_layout, query, entity_path, &image);

    Some(())
}

/// Show the image.
fn image_preview_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    image: &ImageInfo,
) -> Option<()> {
    let render_ctx = ctx.render_ctx?;
    let image_stats = ctx.cache.entry(|c: &mut ImageStatsCache| c.entry(image));
    let annotations = crate::annotations(ctx, query, entity_path);
    let debug_name = entity_path.to_string();
    let texture = image_to_gpu(render_ctx, &debug_name, image, &image_stats, &annotations).ok()?;

    texture_preview_ui(render_ctx, ui, ui_layout, entity_path, texture);

    Some(())
}
