use egui::{Rangef, RichText};
use std::collections::BTreeMap;

use re_chunk_store::UnitChunkShared;
use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_types::{
    ArchetypeName, Component, ComponentDescriptor, archetypes, components,
    datatypes::{ChannelDatatype, ColorModel},
    image::ImageKind,
};
use re_ui::{UiExt as _, design_tokens_of_visuals, list_item};
use re_viewer_context::{
    ColormapWithRange, HoverHighlight, ImageInfo, ImageStatsCache, Item, UiLayout, ViewerContext,
    gpu_bridge::image_data_range_heuristic,
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

        // NOTE: the passed in `db` is usually the recording; NOT the blueprint.
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
        let Some(unordered_components) = component else {
            // This is fine - e.g. we're looking at `/world` and the user has only logged to `/world/car`.
            ui_layout.label(
                ui,
                format!(
                    "{self} has no components on timeline {:?}",
                    query.timeline()
                ),
            );
            return;
        };

        let components_by_archetype = crate::sorted_component_list_by_archetype_for_ui(
            ctx.reflection(),
            &unordered_components,
        );

        let mut query_results =
            db.storage_engine()
                .cache()
                .latest_at(query, entity_path, &unordered_components);

        // Keep previously established order.
        let mut components_by_archetype: BTreeMap<
            Option<ArchetypeName>,
            Vec<(ComponentDescriptor, UnitChunkShared)>,
        > = components_by_archetype
            .into_iter()
            .map(|(archetype, components)| {
                (
                    archetype,
                    components
                        .into_iter()
                        .filter_map(|c| query_results.components.remove(&c).map(|chunk| (c, chunk)))
                        .collect(),
                )
            })
            .collect();

        if components_by_archetype.is_empty() {
            let typ = db.timeline_type(&query.timeline());
            ui_layout.label(
                ui,
                format!(
                    "Nothing logged at {} = {}",
                    query.timeline(),
                    typ.format(query.at(), ctx.app_options().timestamp_format),
                ),
            );
            return;
        }

        if ui_layout.is_single_line() {
            let archetype_count = components_by_archetype.len();
            let component_count = unordered_components.len();
            ui_layout.label(
                ui,
                format!(
                    "{} archetype{} with {} total component{}",
                    archetype_count,
                    if archetype_count > 1 { "s" } else { "" },
                    component_count,
                    if component_count > 1 { "s" } else { "" }
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
                for components in components_by_archetype.values_mut() {
                    components.retain(|(component, _chunk)| {
                        component.component_name != Some(components::GraphEdge::name())
                    });
                }
            }

            component_list_ui(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                instance,
                &components_by_archetype,
            );
        }

        if instance.is_all() {
            // There are some examples where we need to combine several archetypes for a single preview.
            // For instance `VideoFrameReference` and `VideoAsset` are used together for a single preview.
            let components = components_by_archetype
                .values()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            preview_if_image_ui(ctx, ui, ui_layout, query, entity_path, &components);
            preview_if_blob_ui(ctx, ui, ui_layout, query, entity_path, &components);
        }
    }
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
    components: &BTreeMap<Option<ArchetypeName>, Vec<(ComponentDescriptor, UnitChunkShared)>>,
) {
    let interactive = ui_layout != UiLayout::Tooltip;

    re_ui::list_item::list_item_scope(
        ui,
        egui::Id::from("component list").with(entity_path),
        |ui| {
            for (archetype, components) in components {
                ui.list_item()
                    .with_y_offset(1.0)
                    .with_height(20.0)
                    .interactive(false)
                    .show_flat(
                        ui,
                        list_item::LabelContent::new(
                            RichText::new(format!(
                                "{}:",
                                archetype
                                    .map(|a| a.short_name())
                                    .unwrap_or("Without Archetype")
                            ))
                            .size(10.0)
                            .color(design_tokens_of_visuals(ui.visuals()).list_item_strong_text),
                        ),
                    );

                for (component_descr, unit) in components {
                    let component_path =
                        ComponentPath::new(entity_path.clone(), component_descr.clone());

                    let is_static = db
                        .storage_engine()
                        .store()
                        .entity_has_static_component(entity_path, component_descr);
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

                    let content = re_ui::list_item::PropertyContent::new(
                        component_descr.archetype_field_name.as_str(),
                    )
                    .with_icon(icon)
                    .value_fn(|ui, _| {
                        if instance.is_all() {
                            crate::ComponentPathLatestAtResults {
                                component_path: ComponentPath::new(
                                    entity_path.clone(),
                                    component_descr.clone(),
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
                            ctx.component_ui_registry().component_ui(
                                ctx,
                                ui,
                                UiLayout::List,
                                query,
                                db,
                                entity_path,
                                component_descr,
                                unit,
                                instance,
                            );
                        }
                    });

                    let response = list_item.show_flat(ui, content).on_hover_ui(|ui| {
                        if let Some(component_name) = component_descr.component_name {
                            component_name.data_ui_recording(ctx, ui, UiLayout::Tooltip);
                        }

                        if let Some(data) = unit.component_batch_raw(component_descr) {
                            re_ui::list_item::list_item_scope(ui, component_descr, |ui| {
                                ui.list_item_flat_noninteractive(
                                    re_ui::list_item::PropertyContent::new("Data type").value_text(
                                        re_arrow_util::format_data_type(data.data_type()),
                                    ),
                                );
                            });
                        }
                    });

                    if interactive {
                        ctx.handle_select_hover_drag_interactions(&response, item, false);
                    }
                }
            }
        },
    );
}

/// If this entity is an image, show it together with buttons to download and copy the image.
///
/// Expected to get a list of all components on the entity, not just the blob.
fn preview_if_image_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    components: &[(ComponentDescriptor, UnitChunkShared)],
) {
    // There might be several image buffers!
    for (image_buffer_descr, image_buffer_chunk) in components
        .iter()
        .filter(|(descr, _chunk)| descr.component_name == Some(components::ImageBuffer::name()))
    {
        preview_single_image(
            ctx,
            ui,
            ui_layout,
            query,
            entity_path,
            components,
            image_buffer_descr,
            image_buffer_chunk,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn preview_single_image(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    components: &[(ComponentDescriptor, UnitChunkShared)],
    image_buffer_descr: &ComponentDescriptor,
    image_buffer_chunk: &UnitChunkShared,
) -> Option<()> {
    let blob_row_id = image_buffer_chunk.row_id()?;
    let image_buffer = image_buffer_chunk
        .component_mono::<components::ImageBuffer>(image_buffer_descr)?
        .ok()?;

    let (image_format_descr, image_format_chunk) = components.iter().find(|(descr, _chunk)| {
        descr.component_name == Some(components::ImageFormat::name())
            && descr.archetype_name == image_buffer_descr.archetype_name
    })?;
    let image_format = image_format_chunk
        .component_mono::<components::ImageFormat>(image_format_descr)?
        .ok()?;

    let kind = ImageKind::from_archetype_name(image_format_descr.archetype_name);
    let image = ImageInfo::from_stored_blob(
        blob_row_id,
        image_buffer_descr,
        image_buffer.0,
        image_format.0,
        kind,
    );
    let image_stats = ctx
        .store_context
        .caches
        .entry(|c: &mut ImageStatsCache| c.entry(&image));

    let colormap = find_and_deserialize_archetype_mono_component::<components::Colormap>(
        components,
        image_buffer_descr.archetype_name,
    );
    let value_range = find_and_deserialize_archetype_mono_component::<components::ValueRange>(
        components,
        image_buffer_descr.archetype_name,
    );

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
        return Some(());
    }

    let data_range = value_range.map_or_else(
        || image_data_range_heuristic(&image_stats, &image.format),
        |r| Rangef::new(r.start() as _, r.end() as _),
    );
    ui.horizontal(|ui| {
        image_download_button_ui(ctx, ui, entity_path, &image, data_range);

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
                ctx.command_sender().save_file_dialog(
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
                "bar_chart",
                histogram
                    .into_iter()
                    .enumerate()
                    .map(|(i, count)| {
                        Bar::new(i as _, count as _)
                            .width(1.0) // no gaps between bars
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

/// If this entity has a blob, preview it and show a download button.
fn preview_if_blob_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    components: &[(ComponentDescriptor, UnitChunkShared)],
) {
    // There might be several blobs, all with different meanings.
    for (blob_descr, blob_chunk) in components
        .iter()
        .filter(|(descr, _chunk)| descr.component_name == Some(components::Blob::name()))
    {
        preview_single_blob(
            ctx,
            ui,
            ui_layout,
            query,
            entity_path,
            components,
            blob_descr,
            blob_chunk,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn preview_single_blob(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    components: &[(ComponentDescriptor, UnitChunkShared)],
    blob_descr: &ComponentDescriptor,
    blob_chunk: &UnitChunkShared,
) -> Option<()> {
    let blob = blob_chunk
        .component_mono::<components::Blob>(blob_descr)?
        .ok()?;

    // Media type comes typically alongside the blob in various different archetypes.
    // Look for the one that matches the blob's archetype.
    let media_type = find_and_deserialize_archetype_mono_component::<components::MediaType>(
        components,
        blob_descr.archetype_name,
    )
    .or_else(|| components::MediaType::guess_from_data(&blob));

    // Video timestamp is only relevant here if it comes from a VideoFrameReference archetype.
    // It doesn't show up in the blob's archetype.
    let video_timestamp_descr = archetypes::VideoFrameReference::descriptor_timestamp();
    let video_timestamp = components
        .iter()
        .find_map(|(descr, chunk)| {
            (descr == &video_timestamp_descr).then(|| {
                chunk
                    .component_mono::<components::VideoTimestamp>(&video_timestamp_descr)
                    .and_then(|r| r.ok())
            })
        })
        .flatten();

    blob_preview_and_save_ui(
        ctx,
        ui,
        ui_layout,
        query,
        entity_path,
        blob_descr,
        blob_chunk.row_id(),
        &blob,
        media_type.as_ref(),
        video_timestamp,
    );

    Some(())
}

/// Finds and deserializes the given component type if its descriptor matches the given archetype name.
fn find_and_deserialize_archetype_mono_component<C: Component>(
    components: &[(ComponentDescriptor, UnitChunkShared)],
    archetype_name: Option<ArchetypeName>,
) -> Option<C> {
    components.iter().find_map(|(descr, chunk)| {
        (descr.component_name == Some(C::name()) && descr.archetype_name == archetype_name)
            .then(|| chunk.component_mono::<C>(descr).and_then(|r| r.ok()))
            .flatten()
    })
}
