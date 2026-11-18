use re_data_ui::item_ui;
use re_renderer::external::wgpu;
use re_renderer::renderer::ColormappedTexture;
use re_renderer::resource_managers::GpuTexture2D;
use re_sdk_types::encodings::ColorModel;
use re_sdk_types::image::ImageKind;
use re_sdk_types::tensor_data::TensorElement;
use re_view::{
    TextureInteractionId, paint_zoomed_texture_region_outline, zoomed_texture_region_ui,
};
use re_viewer_context::{AnnotationMap, Annotations, ImageInfo, StoreViewContext, ViewQuery};

use crate::PickableRectSourceData;
use crate::SpaceKind;

pub struct PickedPixelInfo {
    pub source_data: PickableRectSourceData,
    pub texture: ColormappedTexture,
    pub pixel_coordinates: [u32; 2],
}

pub fn textured_rect_hover_ui(
    ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    instance_path: &re_entity_db::InstancePath,
    query: &ViewQuery<'_>,
    spatial_kind: SpaceKind,
    ui_pan_and_zoom_from_ui: egui::emath::RectTransform,
    annotations: &AnnotationMap,
    picked_pixel_info: PickedPixelInfo,
    hover_overlay_index: u32,
) {
    let PickedPixelInfo {
        source_data,
        texture,
        pixel_coordinates,
    } = picked_pixel_info;

    let depth_meter = match &source_data {
        PickableRectSourceData::Image { depth_meter, .. }
        | PickableRectSourceData::Video { depth_meter } => *depth_meter,
        PickableRectSourceData::Placeholder => {
            // No point in zooming into a placeholder!
            return;
        }
    };

    let depth_meter = depth_meter.map(|d| *d.0);

    item_ui::instance_path_button(ctx, ui, Some(query.view_id), instance_path);

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let [w, h] = texture.width_height();
        let (w, h) = (w as f32, h as f32);

        if spatial_kind == SpaceKind::TwoD {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));

            paint_zoomed_texture_region_outline(
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

        let annotations = annotations.find(&instance_path.entity_path);

        let interaction_id = TextureInteractionId {
            entity_path: &instance_path.entity_path,
            interaction_idx: hover_overlay_index,
        };
        let render_ctx = ctx.render_ctx();
        let pixel_value_source = image.map_or(
            PixelValueSource::GpuTexture(&texture.texture),
            PixelValueSource::Image,
        );
        let annotations = annotations.unwrap_or_else(|| Annotations::missing_ref());

        zoomed_texture_region_ui(
            render_ctx,
            ui,
            texture.clone(),
            &interaction_id,
            [pixel_coordinates[0] as _, pixel_coordinates[1] as _],
            &|ui| {
                pixel_value_ui(
                    render_ctx,
                    ui,
                    &interaction_id,
                    &pixel_value_source,
                    annotations,
                    pixel_coordinates,
                    depth_meter,
                );
            },
        );
    });
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
        ui.label("Position");
        ui.label(format!("{x}, {y}"));
        ui.end_row();

        if let PixelValueSource::Image(image) = &pixel_value_source {
            // Check for annotations on any single-channel image
            if image.kind == ImageKind::Segmentation
                && let Some(raw_value) = image.get_xyc(x, y, 0)
                && let Some(u16_val) = raw_value.try_as_u16()
            {
                ui.label("Label");
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
                show_depth_at_hover(ui, raw_value.as_f64(), meter);
            }

            if let Some((channel, value)) = image.bayer_sample_at(x, y) {
                let color = ["R", "G", "B"][channel];
                ui.label(format!("Bayer ({color}):"));
                ui.monospace(format!("{value: >3}"));
                ui.end_row();
            }
        }

        if let PixelValueSource::GpuTexture(texture) = &pixel_value_source
            && let Some(meter) = meter
            && let Some(raw_value) =
                depth_value_from_gpu_texture(ui.ctx(), render_ctx, texture, interaction_id, [x, y])
        {
            show_depth_at_hover(ui, raw_value, meter);
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

fn show_depth_at_hover(ui: &mut egui::Ui, raw_value: f64, meter: f32) {
    let depth_in_world_units = raw_value / (meter as f64);
    ui.label("Depth");
    ui.monospace(re_format::format_f64(depth_in_world_units));
}

fn format_pixel_value(
    image_kind: ImageKind,
    color_model: ColorModel,
    elements: &[TensorElement],
) -> Option<(String, String)> {
    match image_kind {
        ImageKind::Segmentation | ImageKind::Depth => elements
            .first()
            .map(|v| ("Val".to_owned(), v.format_padded())),

        ImageKind::Color => match color_model {
            ColorModel::L => elements
                .first()
                .map(|v| ("L".to_owned(), v.format_padded())),

            ColorModel::RGB => {
                if let [r, g, b] = elements {
                    match (r, g, b) {
                        (TensorElement::U8(r), TensorElement::U8(g), TensorElement::U8(b)) => {
                            Some((
                                "RGB".to_owned(),
                                format!("{r: >3}, {g: >3}, {b: >3}, #{r:02X}{g:02X}{b:02X}"),
                            ))
                        }
                        _ => Some((
                            "RGB".to_owned(),
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
                            "RGBA".to_owned(),
                            format!(
                                "{r: >3}, {g: >3}, {b: >3}, {a: >3}, #{r:02X}{g:02X}{b:02X}{a:02X}"
                            ),
                        )),
                        _ => Some((
                            "RGBA".to_owned(),
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
                                "BGR".to_owned(),
                                format!("{b: >3}, {g: >3}, {r: >3}, #{b:02X}{g:02X}{r:02X}"),
                            ))
                        }
                        _ => Some((
                            "BGR".to_owned(),
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
                            "BGRA".to_owned(),
                            format!(
                                "{b: >3}, {g: >3}, {r: >3}, {a: >3}, #{b:02X}{g:02X}{r:02X}{a:02X}"
                            ),
                        )),
                        _ => Some((
                            "BGRA".to_owned(),
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

/// Read back raw pixel bytes from a GPU texture at the given coordinates.
///
/// Schedules a 64x64 region readback around the cursor and polls for results
/// from a previous frame. Returns the raw bytes for the single pixel, or `None`
/// if no result is available yet.
fn readback_pixel_from_gpu_texture(
    egui_ctx: &egui::Context,
    render_ctx: &re_renderer::RenderContext,
    texture: &GpuTexture2D,
    interaction_id: &TextureInteractionId<'_>,
    [x, y]: [u32; 2],
) -> Option<Vec<u8>> {
    // TODO(andreas): Should parts of this be a utility in re_renderer?
    // Note that before this was implemented the readback belt was private to `re_renderer` because it is fairly advanced in its usage.

    let bytes_per_pixel: u32 = texture.format().block_copy_size(None)?;

    let readback_id = interaction_id.gpu_readback_id();

    #[expect(clippy::cast_possible_wrap)]
    let pixel_pos = glam::IVec2::new(x as i32, y as i32);

    let mut readback_belt = render_ctx.gpu_readback_belt.lock();

    // First check if we have a result ready to read.
    // Keep in mind that copy operation may have required row-padding, use `buffer_info` to get the right values.
    // Readbacks from GPU might come in bursts for all sort of reasons. So make sure we only look at the latest result.
    let readback_result = readback_belt.readback_newest_available(
        readback_id,
        |data, userdata: Box<TextureReadbackUserdata>| {
            re_log::debug_assert!(data.len() == userdata.buffer_info.buffer_size_padded as usize);

            // Try to find the pixel at the mouse position.
            // If our position isn't available, just clamp to the edge of the area.
            let data_pos = (pixel_pos - userdata.readback_rect.min())
                .clamp(
                    glam::IVec2::ZERO,
                    // Exclusive the size of the area we're reading back.
                    userdata.readback_rect.extent.as_ivec2() - glam::IVec2::ONE,
                )
                .as_uvec2();
            let start_index = (data_pos.x * bytes_per_pixel
                + userdata.buffer_info.bytes_per_row_padded * data_pos.y)
                as usize;
            let end_index = start_index + bytes_per_pixel as usize;

            data[start_index..end_index].to_vec()
        },
    );

    // Unfortunately, it can happen that GPU readbacks come in bursts one frame and we get thing in the next.
    // Therefore, we have to keep around the previous result and use that until we get a new one.
    let readback_result = {
        let frame_nr = egui_ctx.cumulative_frame_nr();

        #[derive(Clone)]
        struct PreviousReadbackResult {
            frame_nr: u64,
            interaction_id: re_renderer::GpuReadbackIdentifier,
            pixel_bytes: Vec<u8>,
        }

        // Only use the interaction *index* to identify the memory itself so we don't accumulate data indefinitely.
        // To detect whether the retrieved data belongs to the same interaction we add the full interaction *id* to the cached data.
        let memory_id = egui::Id::unique(interaction_id.interaction_idx);
        let interaction_id = interaction_id.gpu_readback_id();

        if let Some(pixel_bytes) = readback_result {
            egui_ctx.memory_mut(|m| {
                m.data.insert_temp(
                    memory_id,
                    PreviousReadbackResult {
                        frame_nr,
                        interaction_id,
                        pixel_bytes: pixel_bytes.clone(),
                    },
                );
            });

            Some(pixel_bytes)
        } else {
            const MAX_FRAMES_WITHOUT_GPU_READBACK: u64 = 3;

            let cached: Option<PreviousReadbackResult> =
                egui_ctx.memory(|m| m.data.get_temp(memory_id));

            cached.and_then(|cached| {
                (cached.interaction_id == interaction_id
                    && cached.frame_nr + MAX_FRAMES_WITHOUT_GPU_READBACK >= frame_nr)
                    .then_some(cached.pixel_bytes)
            })
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
    egui_ctx.request_repaint();

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

    readback_result
}

/// Read back a pixel value from a GPU texture and format it as a string.
fn pixel_value_string_from_gpu_texture(
    egui_ctx: &egui::Context,
    render_ctx: &re_renderer::RenderContext,
    texture: &GpuTexture2D,
    interaction_id: &TextureInteractionId<'_>,
    [x, y]: [u32; 2],
) -> Option<(String, String)> {
    let pixel_bytes =
        readback_pixel_from_gpu_texture(egui_ctx, render_ctx, texture, interaction_id, [x, y])?;

    match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm => {
            let elements = [
                TensorElement::U8(pixel_bytes[0]),
                TensorElement::U8(pixel_bytes[1]),
                TensorElement::U8(pixel_bytes[2]),
            ];
            format_pixel_value(ImageKind::Color, ColorModel::RGB, &elements)
        }
        wgpu::TextureFormat::R8Unorm => {
            let elements = [TensorElement::U8(pixel_bytes[0])];
            format_pixel_value(ImageKind::Depth, ColorModel::L, &elements)
        }
        wgpu::TextureFormat::R16Uint => {
            let value = u16::from_le_bytes([pixel_bytes[0], pixel_bytes[1]]);
            let elements = [TensorElement::U16(value)];
            format_pixel_value(ImageKind::Depth, ColorModel::L, &elements)
        }
        wgpu::TextureFormat::R32Float => {
            let value = f32::from_le_bytes([
                pixel_bytes[0],
                pixel_bytes[1],
                pixel_bytes[2],
                pixel_bytes[3],
            ]);
            let elements = [TensorElement::F32(value)];
            format_pixel_value(ImageKind::Depth, ColorModel::L, &elements)
        }
        _ => None,
    }
}

/// Read back a raw depth value from a GPU texture at the given pixel.
///
/// Returns the depth value as f64, suitable for division by `depth_meter`.
pub fn depth_value_from_gpu_texture(
    egui_ctx: &egui::Context,
    render_ctx: &re_renderer::RenderContext,
    texture: &GpuTexture2D,
    interaction_id: &TextureInteractionId<'_>,
    [x, y]: [u32; 2],
) -> Option<f64> {
    let pixel_bytes =
        readback_pixel_from_gpu_texture(egui_ctx, render_ctx, texture, interaction_id, [x, y])?;

    match texture.format() {
        wgpu::TextureFormat::R8Unorm => Some(pixel_bytes[0] as f64),
        wgpu::TextureFormat::R16Uint => {
            Some(u16::from_le_bytes([pixel_bytes[0], pixel_bytes[1]]) as f64)
        }
        wgpu::TextureFormat::R32Float => Some(f32::from_le_bytes([
            pixel_bytes[0],
            pixel_bytes[1],
            pixel_bytes[2],
            pixel_bytes[3],
        ]) as f64),
        _ => None,
    }
}
