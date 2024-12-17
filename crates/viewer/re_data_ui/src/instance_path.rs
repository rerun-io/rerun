use egui::Rangef;
use nohash_hasher::IntMap;

use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_types::{
    archetypes, components,
    datatypes::{ChannelDatatype, ColorModel},
    image::ImageKind,
    static_assert_struct_has_fields, Archetype, Component, ComponentName,
};
use re_ui::UiExt as _;
use re_viewer_context::{
    gpu_bridge::image_data_range_heuristic, ColormapWithRange, HoverHighlight, ImageInfo,
    ImageStatsCache, Item, UiLayout, ViewerContext,
};

use crate::{blob::blob_preview_and_save_ui, image::image_preview_ui};

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

        let component = if ctx.recording().is_known_entity(entity_path) {
            // We are looking at an entity in the recording
            ctx.recording_engine()
                .store()
                .all_components_on_timeline(&query.timeline(), entity_path)
        } else if ctx.blueprint_db().is_known_entity(entity_path) {
            // We are looking at an entity in the blueprint
            ctx.blueprint_db()
                .storage_engine()
                .store()
                .all_components_on_timeline(&query.timeline(), entity_path)
        } else {
            ui.error_label(format!("Unknown entity: {entity_path:?}"));
            return;
        };
        let Some(components) = component else {
            // This is fine - e.g. we're looking at `/world` and the user has only logged to `/world/car`.
            ui_layout.label(
                ui,
                format!(
                    "{self} has no components on timeline {:?}",
                    query.timeline().name()
                ),
            );
            return;
        };

        let components = crate::sorted_component_list_for_ui(&components);
        let indicator_count = components
            .iter()
            .filter(|c| c.is_indicator_component())
            .count();

        let mut components = latest_at(db, query, entity_path, &components);

        if components.is_empty() {
            ui_layout.label(
                ui,
                format!(
                    "Nothing logged at {} = {}",
                    query.timeline().name(),
                    query
                        .timeline()
                        .typ()
                        .format(query.at(), ctx.app_options.time_zone),
                ),
            );
            return;
        }

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
            // TODO(#7026): Instances today are too poorly defined:
            // For many archetypes it makes sense to slice through all their component arrays with the same index.
            // However, there are cases when there are multiple dimensions of slicing that make sense.
            // This is most obvious for meshes & graph nodes where there are different dimensions for vertices/edges/etc.
            //
            // For graph nodes this is particularly glaring since our indicices imply nodes today and
            // unlike with meshes it's very easy to hover & select individual nodes.
            // In order to work around the GraphEdges showing up associated with random nodes, we just hide them here.
            // (this is obviously a hack and these relationships should be formalized such that they are accessible to the UI, see ticket link above)
            if !self.is_all() {
                components
                    .retain(|(component, _chunk)| component != &components::GraphEdge::name());
            }

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
            preview_if_blob_ui(ctx, ui, ui_layout, query, entity_path, &component_map);
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
                db.storage_engine()
                    .cache()
                    .latest_at(query, entity_path, [component_name]);

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
        UiLayout::SelectionPanel => true,
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
                .storage_engine()
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
                            crate::ComponentPathLatestAtResults {
                                component_path: ComponentPath::new(
                                    entity_path.clone(),
                                    component_name,
                                ),
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
                ctx.handle_select_hover_drag_interactions(&response, item, false);
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
    // First check assumptions:
    static_assert_struct_has_fields!(
        archetypes::Image,
        buffer: components::ImageBuffer,
        format: components::ImageFormat
    );
    static_assert_struct_has_fields!(
        archetypes::DepthImage,
        buffer: components::ImageBuffer,
        format: components::ImageFormat
    );
    static_assert_struct_has_fields!(
        archetypes::SegmentationImage,
        buffer: components::ImageBuffer,
        format: components::ImageFormat
    );

    let image_buffer = component_map.get(&components::ImageBuffer::name())?;
    let buffer_row_id = image_buffer.row_id()?;
    let image_buffer = image_buffer
        .component_mono::<components::ImageBuffer>()?
        .ok()?;

    let image_format = component_map
        .get(&components::ImageFormat::name())?
        .component_mono::<components::ImageFormat>()?
        .ok()?;

    // TODO(#8129): it's ugly but indicators are going away next anyway.
    let kind = if component_map.contains_key(&re_types_core::ComponentBatch::name(
        &archetypes::DepthImage::indicator(),
    )) {
        ImageKind::Depth
    } else if component_map.contains_key(&re_types_core::ComponentBatch::name(
        &archetypes::SegmentationImage::indicator(),
    )) {
        ImageKind::Segmentation
    } else {
        ImageKind::Color
    };

    let image = ImageInfo {
        buffer_row_id,
        buffer: image_buffer.0,
        format: image_format.0,
        kind,
    };
    let image_stats = ctx.cache.entry(|c: &mut ImageStatsCache| c.entry(&image));

    let colormap = component_map
        .get(&components::Colormap::name())
        .and_then(|colormap| colormap.component_mono::<components::Colormap>()?.ok());
    let value_range = component_map
        .get(&components::Range1D::name())
        .and_then(|colormap| colormap.component_mono::<components::ValueRange>()?.ok());
    let colormap_with_range = colormap.map(|colormap| ColormapWithRange {
        colormap,
        value_range: value_range
            .map(|r| [r.start() as _, r.end() as _])
            .unwrap_or_else(|| {
                if kind == ImageKind::Depth {
                    ColormapWithRange::default_range_for_depth_images(&image_stats)
                } else {
                    let (min, max) = image_stats.finite_range;
                    [min as _, max as _]
                }
            }),
    });

    image_preview_ui(
        ctx,
        ui,
        ui_layout,
        query,
        entity_path,
        &image,
        colormap_with_range.as_ref(),
    );

    if ui_layout.is_single_line() || ui_layout == UiLayout::Tooltip {
        return Some(()); // no more ui
    }

    let data_range = value_range.map_or_else(
        || image_data_range_heuristic(&image_stats, &image.format),
        |r| Rangef::new(r.start() as _, r.end() as _),
    );
    ui.horizontal(|ui| {
        image_download_button_ui(ctx, ui, entity_path, &image, data_range);

        #[cfg(not(target_arch = "wasm32"))]
        crate::image::copy_image_button_ui(ui, &image, data_range);
    });

    // TODO(emilk): we should really support histograms for all types of images
    if image.format.pixel_format.is_none()
        && image.format.color_model() == ColorModel::RGB
        && image.format.datatype() == ChannelDatatype::U8
    {
        ui.section_collapsing_header("Histogram")
            .default_open(false)
            .show(ui, |ui| {
                rgb8_histogram_ui(ui, &image.buffer);
            });
    }

    Some(())
}

fn image_download_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &re_log_types::EntityPath,
    image: &ImageInfo,
    data_range: egui::Rangef,
) {
    let text = if cfg!(target_arch = "wasm32") {
        "Download image…"
    } else {
        "Save image…"
    };
    if ui.button(text).clicked() {
        match image.to_png(data_range.into()) {
            Ok(png_bytes) => {
                let file_name = format!(
                    "{}.png",
                    entity_path
                        .last()
                        .map_or("image", |name| name.unescaped_str())
                        .to_owned()
                );
                ctx.command_sender.save_file_dialog(
                    re_capabilities::MainThreadToken::from_egui_ui(ui),
                    &file_name,
                    "Save image".to_owned(),
                    png_bytes,
                );
            }
            Err(err) => {
                re_log::error!("{err}");
            }
        }
    }
}

fn rgb8_histogram_ui(ui: &mut egui::Ui, rgb: &[u8]) -> egui::Response {
    use egui::Color32;
    use itertools::Itertools as _;

    re_tracing::profile_function!();

    let mut histograms = [[0_u64; 256]; 3];
    {
        // TODO(emilk): this is slow, so cache the results!
        re_tracing::profile_scope!("build");
        for pixel in rgb.chunks_exact(3) {
            for c in 0..3 {
                histograms[c][pixel[c] as usize] += 1;
            }
        }
    }

    use egui_plot::{Bar, BarChart, Legend, Plot};

    let names = ["R", "G", "B"];
    let colors = [Color32::RED, Color32::GREEN, Color32::BLUE];

    let charts = histograms
        .into_iter()
        .enumerate()
        .map(|(component, histogram)| {
            let fill = colors[component].linear_multiply(0.5);

            BarChart::new(
                histogram
                    .into_iter()
                    .enumerate()
                    .map(|(i, count)| {
                        Bar::new(i as _, count as _)
                            .width(0.9)
                            .fill(fill)
                            .vertical()
                            .stroke(egui::Stroke::NONE)
                    })
                    .collect(),
            )
            .color(colors[component])
            .name(names[component])
        })
        .collect_vec();

    re_tracing::profile_scope!("show");
    Plot::new("rgb_histogram")
        .legend(Legend::default())
        .height(200.0)
        .show_axes([false; 2])
        .show(ui, |plot_ui| {
            for chart in charts {
                plot_ui.bar_chart(chart);
            }
        })
        .response
}

/// If this entity has a blob, preview it and show a download button
fn preview_if_blob_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    component_map: &IntMap<ComponentName, UnitChunkShared>,
) -> Option<()> {
    let blob = component_map.get(&components::Blob::name())?;
    let blob_row_id = blob.row_id();
    let blob = blob.component_mono::<components::Blob>()?.ok()?;
    let media_type = component_map
        .get(&components::MediaType::name())
        .and_then(|unit| unit.component_mono::<components::MediaType>()?.ok())
        .or_else(|| components::MediaType::guess_from_data(&blob));

    let video_timestamp = component_map
        .get(&components::VideoTimestamp::name())
        .and_then(|unit| unit.component_mono::<components::VideoTimestamp>()?.ok());

    blob_preview_and_save_ui(
        ctx,
        ui,
        ui_layout,
        query,
        entity_path,
        blob_row_id,
        &blob,
        media_type.as_ref(),
        video_timestamp,
    );

    Some(())
}
