use egui::{NumExt as _, Rangef, Vec2};
use re_capabilities::MainThreadToken;
use re_chunk_store::UnitChunkShared;
use re_renderer::renderer::ColormappedTexture;
use re_sdk_types::components;
use re_sdk_types::components::MediaType;
use re_sdk_types::datatypes::{ChannelDatatype, ColorModel};
use re_sdk_types::image::ImageKind;
use re_types_core::{Component as _, ComponentDescriptor, RowId};
use re_ui::list_item::ListItemContentButtonsExt as _;
use re_ui::{UiExt as _, icons, list_item};
use re_viewer_context::gpu_bridge::{self, image_data_range_heuristic, image_to_gpu};
use re_viewer_context::{ColormapWithRange, ImageInfo, ImageStatsCache, UiLayout, ViewerContext};

use crate::find_and_deserialize_archetype_mono_component;

/// Show the given image with an appropriate size.
///
/// For segmentation images, the annotation context is looked up.
pub fn image_preview_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    image: &ImageInfo,
    colormap_with_range: Option<&ColormapWithRange>,
) -> Option<()> {
    let image_stats = ctx
        .store_context
        .caches
        .entry(|c: &mut ImageStatsCache| c.entry(image));
    let annotations = crate::annotations(ctx, query, entity_path);
    let debug_name = entity_path.to_string();
    let texture = image_to_gpu(
        ctx.render_ctx(),
        &debug_name,
        image,
        &image_stats,
        &annotations,
        colormap_with_range,
    )
    .ok()?;

    let [w, h] = texture.width_height();
    let preview_size = texture_preview_size(ui, ui_layout, [w, h]);

    texture_preview_ui(
        ctx.render_ctx(),
        ui,
        ui_layout,
        &debug_name,
        texture,
        preview_size,
    );

    Some(())
}

pub fn texture_preview_size(ui: &egui::Ui, ui_layout: UiLayout, texture_size: [u32; 2]) -> Vec2 {
    let [texture_width, texture_height] = texture_size;
    let max_size = if ui_layout.is_single_line() {
        let height = ui.available_height();
        let width =
            (height * texture_width as f32 / texture_height as f32).at_most(ui.available_width());
        Vec2::new(width, height)
    } else {
        // TODO(emilk): we should limit the HEIGHT primarily,
        // since if the image uses up too much vertical space,
        // it is really annoying in the selection panel.
        let size_range = if ui_layout == UiLayout::Tooltip {
            egui::Rangef::new(64.0, 128.0)
        } else {
            egui::Rangef::new(240.0, 320.0)
        };
        Vec2::splat(
            size_range
                .clamp(ui.available_width())
                .at_most(16.0 * texture_width.max(texture_height) as f32),
        )
    };

    largest_size_that_fits_in(texture_width as f32 / texture_height as f32, max_size)
}

/// Show the given texture with an appropriate size.
pub fn texture_preview_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    debug_name: &str,
    texture: ColormappedTexture,
    preview_size: Vec2,
) -> egui::Response {
    if ui_layout.is_single_line() {
        ui.allocate_ui_with_layout(
            preview_size,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.set_min_size(preview_size);

                match show_image_preview(render_ctx, ui, texture.clone(), debug_name, preview_size)
                {
                    Ok(response) => response.on_hover_ui(|ui| {
                        // Show larger image on hover.
                        let hover_size = Vec2::splat(400.0);
                        show_image_preview(render_ctx, ui, texture, debug_name, hover_size).ok();
                    }),
                    Err((response, err)) => response.on_hover_text(err.to_string()),
                }
            },
        )
        .inner
    } else {
        show_image_preview(render_ctx, ui, texture, debug_name, preview_size).unwrap_or_else(
            |(response, err)| {
                re_log::warn_once!("Failed to show texture {debug_name}: {err}");
                response
            },
        )
    }
}

/// Shows preview of an image.
///
/// Displays the image at the desired size, without overshooting it, and preserving aspect ration.
///
/// Extremely small images will be stretched on their thin axis to make them visible.
/// This does not preserve aspect ratio, but we only stretch it to a very thin size, so it is fine.
///
/// Returns error if the image could not be rendered.
fn show_image_preview(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormapped_texture: ColormappedTexture,
    debug_name: &str,
    desired_size: egui::Vec2,
) -> Result<egui::Response, (egui::Response, anyhow::Error)> {
    fn texture_size(colormapped_texture: &ColormappedTexture) -> Vec2 {
        let [w, h] = colormapped_texture.width_height();
        egui::vec2(w as f32, h as f32)
    }

    const MIN_SIZE: f32 = 2.0;

    let texture_size = texture_size(&colormapped_texture);

    let scaled_size = largest_size_that_fits_in(texture_size.x / texture_size.y, desired_size);

    // Don't allow images so thin that we cannot see them:
    let scaled_size = scaled_size.max(Vec2::splat(MIN_SIZE));

    let (response, painter) = ui.allocate_painter(scaled_size, egui::Sense::hover());

    // Place it in the center:
    let texture_rect_on_screen = egui::Rect::from_center_size(response.rect.center(), scaled_size);

    if let Err(err) = gpu_bridge::render_image(
        render_ctx,
        &painter,
        texture_rect_on_screen,
        colormapped_texture,
        egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
            ..Default::default()
        },
        debug_name.into(),
        None,
    ) {
        let color = ui.visuals().error_fg_color;
        painter.text(
            response.rect.left_top(),
            egui::Align2::LEFT_TOP,
            "🚫",
            egui::FontId::default(),
            color,
        );
        Err((response, err))
    } else {
        Ok(response)
    }
}

fn largest_size_that_fits_in(aspect_ratio: f32, max_size: Vec2) -> Vec2 {
    if aspect_ratio < max_size.x / max_size.y {
        // A thin image in a landscape frame
        egui::vec2(aspect_ratio * max_size.y, max_size.y)
    } else {
        // A wide image in a portrait frame
        egui::vec2(max_size.x, max_size.x / aspect_ratio)
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

pub struct ImageUi {
    image: ImageInfo,
    data_range: Rangef,
    colormap_with_range: Option<ColormapWithRange>,
}

impl ImageUi {
    pub fn new(ctx: &ViewerContext<'_>, image: ImageInfo) -> Self {
        let image_stats = ctx
            .store_context
            .caches
            .entry(|c: &mut ImageStatsCache| c.entry(&image));
        let data_range = image_data_range_heuristic(&image_stats, &image.format);
        Self {
            image,
            data_range,
            colormap_with_range: None,
        }
    }

    pub fn from_blob(
        ctx: &ViewerContext<'_>,
        blob_row_id: RowId,
        blob_component_descriptor: &ComponentDescriptor,
        blob: &re_sdk_types::datatypes::Blob,
        media_type: Option<&MediaType>,
    ) -> Option<Self> {
        ctx.store_context
            .caches
            .entry(|c: &mut re_viewer_context::ImageDecodeCache| {
                c.entry_encoded_color(
                    blob_row_id,
                    blob_component_descriptor.component,
                    blob,
                    media_type,
                )
            })
            .ok()
            .map(|image| Self::new(ctx, image))
    }

    pub fn from_components(
        ctx: &ViewerContext<'_>,
        image_buffer_descr: &ComponentDescriptor,
        image_buffer_chunk: &UnitChunkShared,
        entity_components: &[(ComponentDescriptor, UnitChunkShared)],
    ) -> Option<Self> {
        if image_buffer_descr.component_type != Some(components::ImageBuffer::name()) {
            return None;
        }

        let blob_row_id = image_buffer_chunk.row_id()?;
        let image_buffer = image_buffer_chunk
            .component_mono::<components::ImageBuffer>(image_buffer_descr.component)?
            .ok()?;

        let (image_format_descr, image_format_chunk) =
            entity_components.iter().find(|(descr, _chunk)| {
                descr.component_type == Some(components::ImageFormat::name())
                    && descr.archetype == image_buffer_descr.archetype
            })?;
        let image_format = image_format_chunk
            .component_mono::<components::ImageFormat>(image_format_descr.component)?
            .ok()?;

        let kind = ImageKind::from_archetype_name(image_format_descr.archetype);
        let image = ImageInfo::from_stored_blob(
            blob_row_id,
            image_buffer_descr.component,
            image_buffer.0,
            image_format.0,
            kind,
        );
        let image_stats = ctx
            .store_context
            .caches
            .entry(|c: &mut ImageStatsCache| c.entry(&image));

        let colormap = find_and_deserialize_archetype_mono_component::<components::Colormap>(
            entity_components,
            image_buffer_descr.archetype,
        );
        let value_range = find_and_deserialize_archetype_mono_component::<components::ValueRange>(
            entity_components,
            image_buffer_descr.archetype,
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

        let data_range = value_range.map_or_else(
            || image_data_range_heuristic(&image_stats, &image.format),
            |r| Rangef::new(r.start() as _, r.end() as _),
        );

        Some(Self {
            image,
            data_range,
            colormap_with_range,
        })
    }

    pub fn inline_copy_button<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
        property_content: list_item::PropertyContent<'a>,
    ) -> list_item::PropertyContent<'a> {
        property_content.with_action_button(&icons::COPY, "Copy image", move || {
            if let Some(rgba) = self.image.to_rgba8_image(self.data_range.into()) {
                let egui_image = egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as _, rgba.height() as _],
                    bytemuck::cast_slice(rgba.as_raw()),
                );
                ctx.egui_ctx().copy_image(egui_image);
                re_log::info!("Copied image to clipboard");
            } else {
                re_log::error!("Invalid image");
            }
        })
    }

    pub fn inline_download_button<'a>(
        &'a self,
        ctx: &'a ViewerContext<'_>,
        main_thread_token: MainThreadToken,
        entity_path: &'a re_log_types::EntityPath,
        property_content: list_item::PropertyContent<'a>,
    ) -> list_item::PropertyContent<'a> {
        property_content.with_action_button(&icons::DOWNLOAD, "Save image", move || {
            match self.image.to_png(self.data_range.into()) {
                Ok(png_bytes) => {
                    let file_name = format!(
                        "{}.png",
                        entity_path
                            .last()
                            .map_or("image", |name| name.unescaped_str())
                            .to_owned()
                    );
                    ctx.command_sender().save_file_dialog(
                        main_thread_token,
                        &file_name,
                        "Save image".to_owned(),
                        png_bytes,
                    );
                }
                Err(err) => {
                    re_log::error!("{err}");
                }
            }
        })
    }

    pub fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &re_log_types::EntityPath,
    ) {
        let Self {
            image,
            data_range: _,
            colormap_with_range,
        } = self;

        image_preview_ui(
            ctx,
            ui,
            ui_layout,
            query,
            entity_path,
            image,
            colormap_with_range.as_ref(),
        );

        if ui_layout.is_single_line() || ui_layout == UiLayout::Tooltip {
            return;
        }

        ui.list_item_flat_noninteractive(
            list_item::PropertyContent::new("Image format").value_text(image.format.to_string()),
        );

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
    }
}
