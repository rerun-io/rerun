use re_renderer::renderer::ColormappedTexture;
use re_ui::UiExt as _;
use re_viewer_context::gpu_bridge;

/// Number of texels shown on each side of the center texel by [`zoomed_texture_region_ui`].
const ZOOMED_IMAGE_TEXEL_RADIUS: i64 = 10;

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
    pub fn debug_label(&self, topic: &str) -> re_renderer::Label {
        format!("{topic}__{:?}_{}", self.entity_path, self.interaction_idx).into()
    }

    pub fn gpu_readback_id(&self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash((self.entity_path, self.interaction_idx)).hash64()
    }

    fn render_view_id(&self, topic: &str) -> re_renderer::ViewBuilderId {
        re_renderer::ViewBuilderId::new(
            re_log_types::hash::Hash64::hash((self.entity_path, self.interaction_idx, topic))
                .hash64(),
        )
    }
}

/// Draws a border on the original image around the area zoomed in by [`zoomed_texture_region_ui`].
///
/// `image_rect` is where the whole texture is painted on screen, and `ui_clip_rect` clips the border.
pub fn paint_zoomed_texture_region_outline(
    egui_ctx: &egui::Context,
    ui_clip_rect: egui::Rect,
    texture_resolution: egui::Vec2,
    [center_x, center_y]: [i64; 2],
    image_rect: egui::Rect,
) {
    use egui::{Rect, pos2, remap};

    let width = texture_resolution.x;
    let height = texture_resolution.y;

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

/// Shows a magnified view of the texels around `center_texel`.
///
/// If `center_texel` lies inside the texture, `value_ui` is shown next to the magnified view,
/// followed by a large swatch of the center texel's color.
pub fn zoomed_texture_region_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormapped_texture: ColormappedTexture,
    interaction_id: &TextureInteractionId<'_>,
    center_texel: [i64; 2],
    value_ui: &dyn Fn(&mut egui::Ui),
) {
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

        if let Err(err) = gpu_bridge::render_image(
            render_ctx,
            &painter.with_clip_rect(zoom_rect),
            image_rect_on_screen,
            colormapped_texture.clone(),
            egui::TextureOptions::NEAREST,
            interaction_id.render_view_id("zoomed_region"),
            interaction_id.debug_label("zoomed_region"),
        ) {
            ui.error_with_details_on_hover(err.to_string());
            return;
        }
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

            value_ui(ui);

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
            if let Err(err) = gpu_bridge::render_image(
                render_ctx,
                &ui.painter().with_clip_rect(rect),
                image_rect_on_screen,
                colormapped_texture,
                egui::TextureOptions::NEAREST,
                interaction_id.render_view_id("single_pixel"),
                interaction_id.debug_label("single_pixel"),
            ) {
                ui.error_with_details_on_hover(err.to_string());
            }
        });
    }
}
