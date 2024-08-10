use egui::{NumExt, Vec2};

use re_log::ResultExt;
use re_renderer::renderer::ColormappedTexture;
use re_types::components::{Blob, MediaType};
use re_ui::{list_item::PropertyContent, UiExt as _};
use re_viewer_context::{gpu_bridge::image_to_gpu, UiLayout};

use crate::{image::show_image_preview, EntityDataUi};

impl EntityDataUi for Blob {
    fn entity_data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &re_log_types::EntityPath,
        row_id: Option<re_chunk_store::RowId>,
        query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let compact_size_string = re_format::format_bytes(self.len() as _);

        // We show the actual mime of the blob here instead of doing
        // a side-lookup of the sibling `MediaType` component.
        // This is part of "showing the data as it is".
        // If the user clicked on the blob, is because they want to see info about the blob,
        // not about a sibling component.
        // This can also help a user debug if they log the contents of `.png` file with a `image/jpeg` `MediaType`.
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
                show_image_preview(render_ctx, ui, texture.clone(), &debug_name, preview_size)
                    .unwrap_or_else(|(response, err)| {
                        re_log::warn_once!("Failed to show texture {entity_path}: {err}");
                        response
                    });
            }

            if ui_layout != UiLayout::Tooltip {
                let text = if cfg!(target_arch = "wasm32") {
                    "Download blob…"
                } else {
                    "Save blob to file…"
                };
                if ui.button(text).clicked() {
                    let mut file_name = entity_path
                        .last()
                        .map_or("blob", |name| name.unescaped_str())
                        .to_owned();

                    if let Some(file_extension) =
                        media_type.as_ref().and_then(|mt| mt.file_extension())
                    {
                        file_name.push('.');
                        file_name.push_str(file_extension);
                    }

                    save_blob(ctx, file_name, "Save blob".to_owned(), self.clone())
                        .ok_or_log_error();
                }
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
    blob: &Blob,
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

#[allow(clippy::needless_pass_by_ref_mut)] // `app` is only used on native
#[allow(clippy::unnecessary_wraps)] // cannot return error on web
fn save_blob(
    #[allow(unused_variables)] ctx: &re_viewer_context::ViewerContext<'_>, // only used on native
    file_name: String,
    title: String,
    blob: Blob,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    // Web
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = async_save_dialog(&file_name, &title, blob).await {
                re_log::error!("File saving failed: {err}");
            }
        });
    }

    // Native
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = {
            re_tracing::profile_scope!("file_dialog");
            rfd::FileDialog::new()
                .set_file_name(file_name)
                .set_title(title)
                .save_file()
        };
        if let Some(path) = path {
            use re_viewer_context::SystemCommandSender as _;
            ctx.command_sender
                .send_system(re_viewer_context::SystemCommand::FileSaver(Box::new(
                    move || {
                        std::fs::write(&path, blob.as_slice())?;
                        Ok(path)
                    },
                )));
        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn async_save_dialog(file_name: &str, title: &str, data: Blob) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let file_handle = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .set_title(title)
        .save_file()
        .await;

    let Some(file_handle) = file_handle else {
        return Ok(()); // aborted
    };

    file_handle
        .write(&data.as_slice())
        .await
        .context("Failed to save")
}
