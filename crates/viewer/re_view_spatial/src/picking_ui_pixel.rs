use re_data_ui::item_ui;
use re_renderer::external::wgpu;
use re_renderer::renderer::ColormappedTexture;
use re_renderer::resource_managers::GpuTexture2D;
use re_sdk_types::datatypes::ColorModel;
use re_sdk_types::image::ImageKind;
use re_sdk_types::tensor_data::TensorElement;
use re_ui::UiExt as _;
use re_view::AnnotationSceneContext;
use re_viewer_context::{Annotations, ImageInfo, ViewQuery, ViewerContext, gpu_bridge};

use crate::PickableRectSourceData;
use crate::view_kind::SpatialViewKind;

pub struct PickedPixelInfo {
    pub source_data: PickableRectSourceData,
    pub texture: ColormappedTexture,
    pub pixel_coordinates: [u32; 2],
}

#[expect(clippy::too_many_arguments)]
pub fn textured_rect_hover_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_path: &re_entity_db::InstancePath,
    query: &ViewQuery<'_>,
    spatial_kind: SpatialViewKind,
    ui_pan_and_zoom_from_ui: egui::emath::RectTransform,
    annotations: &AnnotationSceneContext,
    picked_pixel_info: PickedPixelInfo,
    hover_overlay_index: u32,
) {
    let PickedPixelInfo {
        source_data,
        texture,
        pixel_coordinates,
    } = picked_pixel_info;

    let depth_meter = match &source_data {
        PickableRectSourceData::Image { depth_meter, .. } => *depth_meter,
        PickableRectSourceData::Video => None,
        PickableRectSourceData::Placeholder => {
            // No point in zooming into a placeholder!
            return;
        }
    };

    let depth_meter = depth_meter.map(|d| *d.0);

    item_ui::instance_path_button(
        ctx,
        &query.latest_at_query(),
        ctx.recording(),
        ui,
        Some(query.view_id),
        instance_path,
    );

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let [w, h] = texture.width_height();
        let (w, h) = (w as f32, h as f32);

        if spatial_kind == SpatialViewKind::TwoD {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));

            show_zoomed_image_region_area_outline(
                ui.ctx(),
                *ui_pan_and_zoom_from_ui.from(),
                egui::vec2(w, h),
                [pixel_coordinates[0] as _, pixel_coordinates[1] as _],
                ui_pan_and_zoom_from_ui.inverse().transform_rect(rect),
            );
        }

        let image = if let PickableRectSourceData::Image { image, .. } = &source_data {
            Some(image)
        } else {
            None
        };

        let annotations = annotations.0.find(&instance_path.entity_path);

        show_zoomed_image_region(
            ctx.render_ctx(),
            ui,
            texture,
            image,
            &annotations,
            depth_meter,
            &TextureInteractionId {
                entity_path: &instance_path.entity_path,
                interaction_idx: hover_overlay_index,
            },
            [pixel_coordinates[0] as _, pixel_coordinates[1] as _],
        );
    });
}

// Show the surrounding pixels:
const ZOOMED_IMAGE_TEXEL_RADIUS: i64 = 10;

/// Draws a border for the area zoomed in by [`show_zoomed_image_region`].
fn show_zoomed_image_region_area_outline(
    egui_ctx: &egui::Context,
    ui_clip_rect: egui::Rect,
    image_resolution: egui::Vec2,
    [center_x, center_y]: [i64; 2],
    image_rect: egui::Rect,
) {
    use egui::{Rect, pos2, remap};

    let width = image_resolution.x;
    let height = image_resolution.y;

    // Show where on the original image the zoomed-in region is at:
    // The area shown is ZOOMED_IMAGE_TEXEL_RADIUS _surrounding_ the center.
    // Since the center is the top-left/rounded-down, coordinate, we need to add 1 to right/bottom.
    let left = (center_x - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let right = (center_x + ZOOMED_IMAGE_TEXEL_RADIUS + 1) as f32;
    let top = (center_y - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let bottom = (center_y + ZOOMED_IMAGE_TEXEL_RADIUS + 1) as f32;

    let left = remap(left, 0.0..=width, image_rect.x_range());
    let right = remap(right, 0.0..=width, image_rect.x_range());
    let top = remap(top, 0.0..=height, image_rect.y_range());
    let bottom = remap(bottom, 0.0..=height, image_rect.y_range());

    let sample_rect = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    // TODO(emilk): use `parent_ui.painter()` and put it in a high Z layer, when https://github.com/emilk/egui/issues/1516 is done
    let painter = egui_ctx.debug_painter().with_clip_rect(ui_clip_rect);
    painter.rect_stroke(
        sample_rect,
        0.0,
        (2.0, egui::Color32::BLACK),
        egui::StrokeKind::Middle,
    );
    painter.rect_stroke(
        sample_rect,
        0.0,
        (1.0, egui::Color32::WHITE),
        egui::StrokeKind::Middle,
    );
}

/// Identifies an image/texture interaction.
///
/// This is needed primarily to keep track of gpu readbacks and for debugging purposes.
/// Therefore, this should stay roughly stable over several frames.
pub struct TextureInteractionId<'a> {
    pub entity_path: &'a re_log_types::EntityPath,

    /// Index of the interaction. This is important in case there's multiple interactions with the same entity.
    /// This can happen if an entity has several images all of which are inspected at the same time.
    /// Without this, several readbacks may get the same identifier, resulting in the wrong gpu readback values.
    pub interaction_idx: u32,
}

impl TextureInteractionId<'_> {
    pub fn debug_label(&self, topic: &str) -> re_renderer::DebugLabel {
        format!("{topic}__{:?}_{}", self.entity_path, self.interaction_idx).into()
    }

    pub fn gpu_readback_id(&self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash((self.entity_path, self.interaction_idx)).hash64()
    }
}

/// `meter`: iff this is a depth map, how long is one meter?
#[expect(clippy::too_many_arguments)]
pub fn show_zoomed_image_region(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    texture: ColormappedTexture,
    image: Option<&ImageInfo>,
    annotations: &Annotations,
    meter: Option<f32>,
    interaction_id: &TextureInteractionId<'_>,
    center_texel: [i64; 2],
) {
    if let Err(err) = try_show_zoomed_image_region(
        render_ctx,
        ui,
        image,
        texture,
        annotations,
        meter,
        interaction_id,
        center_texel,
    ) {
        ui.error_with_details_on_hover(err.to_string());
    }
}

/// `meter`: iff this is a depth map, how long is one meter?
#[expect(clippy::too_many_arguments)]
fn try_show_zoomed_image_region(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    image: Option<&ImageInfo>,
    colormapped_texture: ColormappedTexture,
    annotations: &Annotations,
    meter: Option<f32>,
    interaction_id: &TextureInteractionId<'_>,
    center_texel: [i64; 2],
) -> anyhow::Result<()> {
    let [width, height] = colormapped_texture.width_height();

    const POINTS_PER_TEXEL: f32 = 5.0;
    let size = egui::Vec2::splat(((ZOOMED_IMAGE_TEXEL_RADIUS * 2 + 1) as f32) * POINTS_PER_TEXEL);

    let (_id, zoom_rect) = ui.allocate_space(size);
    let painter = ui.painter();

    painter.rect_filled(zoom_rect, 0.0, ui.visuals().extreme_bg_color);

    let center_of_center_texel = egui::vec2(
        (center_texel[0] as f32) + 0.5,
        (center_texel[1] as f32) + 0.5,
    );

    // Paint the zoomed in region:
    {
        let image_rect_on_screen = egui::Rect::from_min_size(
            zoom_rect.center() - POINTS_PER_TEXEL * center_of_center_texel,
            POINTS_PER_TEXEL * egui::vec2(width as f32, height as f32),
        );

        gpu_bridge::render_image(
            render_ctx,
            &painter.with_clip_rect(zoom_rect),
            image_rect_on_screen,
            colormapped_texture.clone(),
            egui::TextureOptions::NEAREST,
            interaction_id.debug_label("zoomed_region"),
        )?;
    }

    // Outline the center texel, to indicate which texel we're printing the values of:
    {
        let center_texel_rect =
            egui::Rect::from_center_size(zoom_rect.center(), egui::Vec2::splat(POINTS_PER_TEXEL));
        painter.rect_stroke(
            center_texel_rect.expand(1.0),
            0.0,
            (1.0, egui::Color32::BLACK),
            egui::StrokeKind::Outside,
        );
        painter.rect_stroke(
            center_texel_rect,
            0.0,
            (1.0, egui::Color32::WHITE),
            egui::StrokeKind::Outside,
        );
    }

    let [x, y] = center_texel;
    if 0 <= x && (x as u32) < width && 0 <= y && (y as u32) < height {
        ui.separator();

        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            pixel_value_ui(
                render_ctx,
                ui,
                interaction_id,
                &image.map_or(
                    PixelValueSource::GpuTexture(&colormapped_texture.texture),
                    PixelValueSource::Image,
                ),
                annotations,
                [x as _, y as _],
                meter,
            );

            // Show a big sample of the color of the middle texel:
            let (rect, _) = ui.allocate_exact_size(
                egui::Vec2::splat(ui.available_height()),
                egui::Sense::hover(),
            );
            // Position texture so that the center texel is at the center of the rect:
            let zoom = rect.width();
            let image_rect_on_screen = egui::Rect::from_min_size(
                rect.center() - zoom * center_of_center_texel,
                zoom * egui::vec2(width as f32, height as f32),
            );
            gpu_bridge::render_image(
                render_ctx,
                &ui.painter().with_clip_rect(rect),
                image_rect_on_screen,
                colormapped_texture,
                egui::TextureOptions::NEAREST,
                interaction_id.debug_label("single_pixel"),
            )
        })
        .inner?;
    }
    Ok(())
}

/// How we figure out what value to show for a single pixel.
enum PixelValueSource<'a> {
    /// Full image information. Use this whenever reasonably possible.
    Image(&'a ImageInfo),

    /// Via a GPU texture readback.
    ///
    /// As of writing, use this only if…
    /// * the texture is known to be able to read back
    /// * the texture format is `Rgba8UnormSrgb`
    /// * you don't care about alpha (since there's no 24bit textures, we assume we can just ignore it)
    ///
    /// Note that these restrictions are not final,
    /// but merely what covers the usecases right now with the least amount of effort.
    GpuTexture(&'a GpuTexture2D),
}

/// Shows the value of a pixel in an image.
/// If no image info is provided, this only shows the position of the pixel.
fn pixel_value_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    interaction_id: &TextureInteractionId<'_>,
    pixel_value_source: &PixelValueSource<'_>,
    annotations: &Annotations,
    [x, y]: [u32; 2],
    meter: Option<f32>,
) {
    egui::Grid::new("hovered pixel properties").show(ui, |ui| {
        ui.label("Position:");
        ui.label(format!("{x}, {y}"));
        ui.end_row();

        if let PixelValueSource::Image(image) = &pixel_value_source {
            // Check for annotations on any single-channel image
            if image.kind == ImageKind::Segmentation
                && let Some(raw_value) = image.get_xyc(x, y, 0)
                && let Some(u16_val) = raw_value.try_as_u16()
            {
                ui.label("Label:");
                ui.label(
                    annotations
                        .resolved_class_description(Some(re_sdk_types::components::ClassId::from(
                            u16_val,
                        )))
                        .annotation_info()
                        .label(None)
                        .unwrap_or_else(|| u16_val.to_string()),
                );
                ui.end_row();
            }

            if let Some(meter) = meter
                && let Some(raw_value) = image.get_xyc(x, y, 0)
            {
                let raw_value = raw_value.as_f64();
                let meters = raw_value / (meter as f64);
                ui.label("Depth:");
                if meters < 1.0 {
                    ui.monospace(format!("{:.1} mm", meters * 1e3));
                } else {
                    ui.monospace(format!("{meters:.3} m"));
                }
            }
        }

        let text = match pixel_value_source {
            PixelValueSource::Image(image) => pixel_value_string_from_image(image, x, y),
            PixelValueSource::GpuTexture(texture) => pixel_value_string_from_gpu_texture(
                ui.ctx(),
                render_ctx,
                texture,
                interaction_id,
                [x, y],
            ),
        };

        if let Some((label, value)) = text {
            ui.label(label);
            ui.monospace(value);
        } else {
            ui.label("No value");
        }
    });
}

fn format_pixel_value(
    image_kind: ImageKind,
    color_model: ColorModel,
    elements: &[TensorElement],
) -> Option<(String, String)> {
    match image_kind {
        ImageKind::Segmentation | ImageKind::Depth => elements
            .first()
            .map(|v| ("Val:".to_owned(), v.format_padded())),

        ImageKind::Color => match color_model {
            ColorModel::L => elements
                .first()
                .map(|v| ("L:".to_owned(), v.format_padded())),

            ColorModel::RGB => {
                if let [r, g, b] = elements {
                    match (r, g, b) {
                        (TensorElement::U8(r), TensorElement::U8(g), TensorElement::U8(b)) => {
                            Some((
                                "RGB:".to_owned(),
                                format!("{r: >3}, {g: >3}, {b: >3}, #{r:02X}{g:02X}{b:02X}"),
                            ))
                        }
                        _ => Some((
                            "RGB:".to_owned(),
                            format!(
                                "{}, {}, {}",
                                r.format_padded(),
                                g.format_padded(),
                                b.format_padded()
                            ),
                        )),
                    }
                } else {
                    None
                }
            }

            ColorModel::RGBA => {
                if let [r, g, b, a] = elements {
                    match (r, g, b, a) {
                        (
                            TensorElement::U8(r),
                            TensorElement::U8(g),
                            TensorElement::U8(b),
                            TensorElement::U8(a),
                        ) => Some((
                            "RGBA:".to_owned(),
                            format!(
                                "{r: >3}, {g: >3}, {b: >3}, {a: >3}, #{r:02X}{g:02X}{b:02X}{a:02X}"
                            ),
                        )),
                        _ => Some((
                            "RGBA:".to_owned(),
                            format!(
                                "{}, {}, {}, {}",
                                r.format_padded(),
                                g.format_padded(),
                                b.format_padded(),
                                a.format_padded()
                            ),
                        )),
                    }
                } else {
                    None
                }
            }

            ColorModel::BGR => {
                if let [b, g, r] = elements {
                    match (b, g, r) {
                        (TensorElement::U8(b), TensorElement::U8(g), TensorElement::U8(r)) => {
                            Some((
                                "BGR:".to_owned(),
                                format!("{b: >3}, {g: >3}, {r: >3}, #{b:02X}{g:02X}{r:02X}"),
                            ))
                        }
                        _ => Some((
                            "BGR:".to_owned(),
                            format!(
                                "{}, {}, {}",
                                b.format_padded(),
                                g.format_padded(),
                                r.format_padded()
                            ),
                        )),
                    }
                } else {
                    None
                }
            }

            ColorModel::BGRA => {
                if let [b, g, r, a] = elements {
                    match (b, g, r, a) {
                        (
                            TensorElement::U8(b),
                            TensorElement::U8(g),
                            TensorElement::U8(r),
                            TensorElement::U8(a),
                        ) => Some((
                            "BGRA:".to_owned(),
                            format!(
                                "{b: >3}, {g: >3}, {r: >3}, {a: >3}, #{b:02X}{g:02X}{r:02X}{a:02X}"
                            ),
                        )),
                        _ => Some((
                            "BGRA:".to_owned(),
                            format!(
                                "{}, {}, {}, {}",
                                b.format_padded(),
                                g.format_padded(),
                                r.format_padded(),
                                a.format_padded()
                            ),
                        )),
                    }
                } else {
                    None
                }
            }
        },
    }
}

fn pixel_value_string_from_image(image: &ImageInfo, x: u32, y: u32) -> Option<(String, String)> {
    match image.kind {
        ImageKind::Segmentation | ImageKind::Depth => format_pixel_value(
            image.kind,
            image.color_model(),
            image.get_xyc(x, y, 0).as_slice(),
        ),

        ImageKind::Color => match image.color_model() {
            ColorModel::L => format_pixel_value(
                image.kind,
                image.color_model(),
                image.get_xyc(x, y, 0).as_slice(),
            ),

            ColorModel::BGR | ColorModel::RGB => format_pixel_value(
                image.kind,
                image.color_model(),
                &[
                    image.get_xyc(x, y, 0)?,
                    image.get_xyc(x, y, 1)?,
                    image.get_xyc(x, y, 2)?,
                ],
            ),

            ColorModel::BGRA | ColorModel::RGBA => format_pixel_value(
                image.kind,
                image.color_model(),
                &[
                    image.get_xyc(x, y, 0)?,
                    image.get_xyc(x, y, 1)?,
                    image.get_xyc(x, y, 2)?,
                    image.get_xyc(x, y, 3)?,
                ],
            ),
        },
    }
}

struct TextureReadbackUserdata {
    /// Rect on the texture that was read back.
    readback_rect: re_renderer::RectInt,

    /// Info about the buffer we're reading back.
    buffer_info: re_renderer::Texture2DBufferInfo,
}

fn pixel_value_string_from_gpu_texture(
    ui_ctx: &egui::Context,
    render_ctx: &re_renderer::RenderContext,
    texture: &GpuTexture2D,
    interaction_id: &TextureInteractionId<'_>,
    [x, y]: [u32; 2],
) -> Option<(String, String)> {
    // TODO(andreas): Should parts of this be a utility in re_renderer?
    // Note that before this was implemented the readback belt was private to `re_renderer` because it is fairly advanced in its usage.

    // Only support Rgb8Unorm textures for now.
    // We could support more here but that needs more handling code and it doesn't look like we have to right now.
    if texture.format() != wgpu::TextureFormat::Rgba8Unorm {
        return None;
    }

    let readback_id = interaction_id.gpu_readback_id();

    #[expect(clippy::cast_possible_wrap)]
    let pixel_pos = glam::IVec2::new(x as i32, y as i32);

    let mut readback_belt = render_ctx.gpu_readback_belt.lock();

    // First check if we have a result ready to read.
    // Keep in mind that copy operation may have required row-padding, use `buffer_info` to get the right values.
    // Readbacks from GPU might come in bursts for all sort of reasons. So make sure we only look at the latest result.
    let readback_result_rgb = readback_belt.readback_newest_available(
        readback_id,
        |data, userdata: Box<TextureReadbackUserdata>| {
            debug_assert!(data.len() == userdata.buffer_info.buffer_size_padded as usize);

            // Try to find the pixel at the mouse position.
            // If our position isn't available, just clamp to the edge of the area.
            let data_pos = (pixel_pos - userdata.readback_rect.min())
                .clamp(
                    glam::IVec2::ZERO,
                    // Exclusive the size of the area we're reading back.
                    userdata.readback_rect.extent.as_ivec2() - glam::IVec2::ONE,
                )
                .as_uvec2();
            let start_index =
                (data_pos.x * 4 + userdata.buffer_info.bytes_per_row_padded * data_pos.y) as usize;

            [
                data[start_index],
                data[start_index + 1],
                data[start_index + 2],
            ]
        },
    );

    // Unfortunately, it can happen that GPU readbacks come in bursts one frame and we get thing in the next.
    // Therefore, we have to keep around the previous result and use that until we get a new one.
    let readback_result_rgb = {
        let frame_nr = ui_ctx.cumulative_frame_nr();

        #[derive(Clone)]
        struct PreviousReadbackResult {
            frame_nr: u64,
            interaction_id: re_renderer::GpuReadbackIdentifier,
            readback_result_rgb: [u8; 3],
        }

        // Only use the interaction *index* to identify the memory itself so we don't accumulate data indefinitely.
        // To detect whether the retrieved data belongs to the same interaction we add the full interaction *id* to the cached data.
        let memory_id = egui::Id::new(interaction_id.interaction_idx);
        let interaction_id = interaction_id.gpu_readback_id();

        if let Some(readback_result_rgb) = readback_result_rgb {
            ui_ctx.memory_mut(|m| {
                m.data.insert_temp(
                    memory_id,
                    PreviousReadbackResult {
                        frame_nr,
                        interaction_id,
                        readback_result_rgb,
                    },
                );
            });

            Some(readback_result_rgb)
        } else {
            const MAX_FRAMES_WITHOUT_GPU_READBACK: u64 = 3;

            let cached: PreviousReadbackResult = ui_ctx.memory(|m| m.data.get_temp(memory_id))?;

            if cached.interaction_id == interaction_id
                && cached.frame_nr + MAX_FRAMES_WITHOUT_GPU_READBACK >= frame_nr
            {
                Some(cached.readback_result_rgb)
            } else {
                None
            }
        }
    };

    // Then enqueue a new readback.
    //
    // It's quite hard to figure out when we no longer have to do this. The criteria would be roughly:
    // * mouse has not moved
    // * since the mouse moved last time we received the result
    // * the result we received is still about the exact same texture _content_
    //      * if it is a video the exact same texture may show a different frame by now
    // So instead we err on the safe side and keep requesting readbacks & frames.
    ui_ctx.request_repaint();

    // Read back a region of a few pixels. Criteria:
    // * moving the mouse doesn't typically immediately end up in a different region, important since readback has a delay
    // * we don't go overboard and read back a ton of data
    // * copy operation doesn't induce a lot of padding overhead due to row padding requirements
    const READBACK_RECT_SIZE: i32 = 64;

    let resolution = glam::UVec2::from_array(texture.width_height()).as_ivec2();
    let readback_rect_min = (pixel_pos - glam::IVec2::splat(READBACK_RECT_SIZE / 2))
        .clamp(glam::IVec2::ZERO, resolution);
    let readback_rect_max = (pixel_pos + glam::IVec2::splat(READBACK_RECT_SIZE / 2))
        .clamp(glam::IVec2::ZERO, resolution);
    let readback_rect_size = readback_rect_max - readback_rect_min;

    if readback_rect_size.x <= 0 || readback_rect_size.y <= 0 {
        return None;
    }
    let readback_area_size = readback_rect_size.as_uvec2();
    let readback_rect = re_renderer::RectInt {
        min: readback_rect_min,
        extent: readback_area_size,
    };

    let buffer_info =
        re_renderer::Texture2DBufferInfo::new(texture.format(), readback_rect.wgpu_extent());

    let mut readback_buffer = readback_belt.allocate(
        &render_ctx.device,
        &render_ctx.gpu_resources.buffers,
        buffer_info.buffer_size_padded,
        readback_id,
        Box::new(TextureReadbackUserdata {
            readback_rect,
            buffer_info,
        }),
    );
    drop(readback_belt);

    {
        let mut encoder = render_ctx.active_frame.before_view_builder_encoder.lock();
        if let Err(err) = readback_buffer.read_texture2d(
            encoder.get(),
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: readback_rect.wgpu_origin(),
                aspect: wgpu::TextureAspect::All,
            },
            readback_rect.wgpu_extent(),
        ) {
            re_log::error_once!("Failed to read back texture: {err}");
        }
    }

    let rgb = readback_result_rgb?;
    let rgb = [
        TensorElement::U8(rgb[0]),
        TensorElement::U8(rgb[1]),
        TensorElement::U8(rgb[2]),
    ];
    format_pixel_value(ImageKind::Color, ColorModel::RGB, &rgb)
}
