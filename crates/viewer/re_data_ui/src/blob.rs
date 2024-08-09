use egui::{NumExt, Vec2};
use re_renderer::renderer::ColormappedTexture;
use re_types::components::MediaType;
use re_ui::{list_item::PropertyContent, UiExt as _};
use re_viewer_context::gpu_bridge::image_to_gpu;

use crate::{image::show_image_preview, EntityDataUi};

impl EntityDataUi for re_types::components::Blob {
    fn entity_data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: re_viewer_context::UiLayout,
        entity_path: &re_log_types::EntityPath,
        row_id: Option<re_chunk_store::RowId>,
        query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let compact_size_string = re_format::format_bytes(self.len() as _);

        // We ignore the logged `MediaType` component, because the user is looking
        // at the blob specifically, not the entity as a whole!
        let media_type = MediaType::guess_from_data(self);

        let texture = blob_as_texture(ctx, query, entity_path, row_id, self, media_type.as_ref());

        if ui_layout.is_single_line() {
            ui.horizontal(|ui| {
                ui.label(compact_size_string);

                if let Some(media_type) = &media_type {
                    ui.label(media_type.to_string())
                        .on_hover_text("Media type (MIME) based on magic header bytes");
                }

                if let (Some(render_ctx), Some(texture)) = (ctx.render_ctx, texture) {
                    // We want all preview images to take up the same amount of space,
                    // no matter what the actual aspect ratio of the images are.
                    let preview_size = Vec2::splat(ui.available_height());
                    let debug_name = entity_path.to_string();
                    show_mini_image_on_same_row(render_ctx, ui, preview_size, texture, &debug_name);
                }
            });
        } else {
            let all_digits_size_string = format!("{} B", re_format::format_uint(self.len()));
            let size_string = if self.len() < 1024 {
                all_digits_size_string
            } else {
                format!("{all_digits_size_string} ({compact_size_string})")
            };

            ui.list_item_flat_noninteractive(PropertyContent::new("Size").value_text(size_string));

            if let Some(media_type) = &media_type {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Media type").value_text(media_type.as_str()),
                )
                .on_hover_text("Media type (MIME) based on magic header bytes");
            } else {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Media type").value_text("?"),
                )
                .on_hover_text("Failed to detect media type (Mime) from magic header bytes");
            }

            if let (Some(render_ctx), Some(texture)) = (ctx.render_ctx, texture) {
                // We want all preview images to take up the same amount of space,
                // no matter what the actual aspect ratio of the images are.
                let preview_size =
                    Vec2::splat(ui.available_width().at_least(240.0)).at_most(Vec2::splat(640.0));
                let debug_name = entity_path.to_string();
                show_image_preview(render_ctx, ui, texture.clone(), &debug_name, preview_size).ok();
            }
        }
    }
}

fn show_mini_image_on_same_row(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    preview_size: Vec2,
    texture: ColormappedTexture,
    debug_name: &str,
) {
    ui.allocate_ui_with_layout(
        preview_size,
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        |ui| {
            ui.set_min_size(preview_size);

            match show_image_preview(render_ctx, ui, texture.clone(), debug_name, preview_size) {
                Ok(response) => response.on_hover_ui(|ui| {
                    // Show larger image on hover.
                    let hover_size = Vec2::splat(400.0);
                    show_image_preview(render_ctx, ui, texture, debug_name, hover_size).ok();
                }),
                Err((response, err)) => response.on_hover_text(err.to_string()),
            }
        },
    );
}

fn blob_as_texture(
    ctx: &re_viewer_context::ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    row_id: Option<re_chunk_store::RowId>,
    blob: &re_types::components::Blob,
    media_type: Option<&MediaType>,
) -> Option<ColormappedTexture> {
    let render_ctx = ctx.render_ctx?;
    let debug_name = entity_path.to_string();

    let image = row_id.and_then(|row_id| {
        ctx.cache
            .entry(|c: &mut re_viewer_context::ImageDecodeCache| {
                c.entry(row_id, blob, media_type.as_ref().map(|mt| mt.as_str()))
            })
            .ok()
    })?;
    let image_stats = ctx
        .cache
        .entry(|c: &mut re_viewer_context::ImageStatsCache| c.entry(&image));
    let annotations = crate::annotations(ctx, query, entity_path);
    image_to_gpu(render_ctx, &debug_name, &image, &image_stats, &annotations).ok()
}
