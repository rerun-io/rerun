//! Compares `re_renderer`'s line rendering against `epaint`'s.
//!
//! `epaint` is the baseline here: it tessellates polylines on the CPU with an analytic
//! one-pixel feather, which is what the plot lines in the viewer are judged against.
//! Every row draws the exact same polyline, once per [`Renderer`], so the three are
//! directly comparable.
//!
//! Three things make line rendering hard, and all of them are varied here:
//! * **Stroke width.** Below one physical pixel a stroke cannot be drawn at full
//!   brightness without looking wider than it is, so it has to be faded instead.
//! * **Control point density.** With many control points per pixel, consecutive
//!   segments and their joints overlap, and any non-analytic coverage compounds into
//!   visible banding.
//! * **How coverage reaches the framebuffer.** `re_renderer` has two line pipelines and
//!   they fail differently, so both are covered; see [`Renderer`].

use re_renderer::{
    Color32, LineDrawableBuilder, QueueableDrawData, RenderContext, Size,
    renderer::LineStripFlags,
    view_builder::{OrthographicCameraMode, Projection, TargetConfiguration, ViewBuilder},
};
use re_test_context::TestContext;

/// Which renderer draws a row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Renderer {
    /// `re_renderer`'s default line pipeline, which resolves coverage with
    /// alpha-to-coverage. Used by the spatial views.
    AlphaToCoverage,

    /// `re_renderer`'s alpha-blended line pipeline
    /// ([`LineDrawableBuilder::enable_alpha_blending`]). Used by the plot lines in the
    /// time series view, so this is the row to compare against `epaint`.
    AlphaBlending,

    /// `epaint`'s CPU tessellator, the baseline.
    Epaint,
}

impl Renderer {
    const ALL: [Self; 3] = [Self::AlphaToCoverage, Self::AlphaBlending, Self::Epaint];

    /// The two that go through `re_renderer`, each in its own view.
    const RE_RENDERER: [Self; 2] = [Self::AlphaToCoverage, Self::AlphaBlending];

    /// Short tag used in the row labels.
    fn label(self) -> &'static str {
        match self {
            Self::AlphaToCoverage => "a2c",
            Self::AlphaBlending => "ab",
            Self::Epaint => "ep",
        }
    }

    /// How a view holding this renderer's lines must be composited.
    ///
    /// The two pipelines need different compositing — alpha-to-coverage output goes
    /// through the workaround in `composite.wgsl`, premultiplied output must not — and a
    /// view carries a single mode, so each pipeline needs a view of its own. Drawing both
    /// into one view silently sends one of them through the wrong path.
    fn blend_with_background(self) -> re_renderer::BlendWithBackground {
        match self {
            Self::AlphaToCoverage => re_renderer::BlendWithBackground::AlphaToCoverage,
            Self::AlphaBlending | Self::Epaint => re_renderer::BlendWithBackground::Premultiplied,
        }
    }
}

/// Stroke widths in ui points, small enough to hit the sub-pixel cases.
const STROKE_WIDTHS_UI: &[f32] = &[0.5, 1.0, 1.5, 3.0];

/// Number of control points per wave.
///
/// The waves are a bit under [`CELL_WIDTH`] ui points wide, so the last entry puts more
/// than three control points on every point, while the first puts one every ~40 points.
const NUM_CONTROL_POINTS: &[usize] = &[5, 30, 600];

const CELL_WIDTH: f32 = 180.0;

/// Horizontal padding inside a cell, so neighboring waves are clearly separate.
const CELL_PADDING_X: f32 = 8.0;

/// Height of one (stroke width, renderer) row.
const ROW_HEIGHT: f32 = 28.0;

/// Room for the `0.5 a2c` style row labels on the left.
const LABEL_WIDTH: f32 = 60.0;

/// Room for the `n = 600` style column headers and the legend on top.
const HEADER_HEIGHT: f32 = 34.0;

/// Peak-to-center amplitude of the waves, in ui points.
const AMPLITUDE: f32 = 9.0;

/// Full periods per wave. A non-integer count so the two ends have different slopes.
const NUM_PERIODS: f32 = 1.25;

const LABEL_COLOR: Color32 = Color32::GRAY;
const LINE_COLOR: Color32 = Color32::WHITE;

/// Plot lines in the viewer use these, so use them here too.
const LINE_FLAGS: LineStripFlags = LineStripFlags::STRIP_FLAGS_OUTWARD_EXTENDING_ROUND_CAPS;

fn total_size() -> egui::Vec2 {
    egui::vec2(
        LABEL_WIDTH + CELL_WIDTH * NUM_CONTROL_POINTS.len() as f32,
        HEADER_HEIGHT + ROW_HEIGHT * (Renderer::ALL.len() * STROKE_WIDTHS_UI.len()) as f32,
    )
}

/// The polyline drawn in the row starting at `top_left`.
fn wave(top_left: egui::Pos2, num_control_points: usize) -> Vec<egui::Pos2> {
    let center_y = top_left.y + ROW_HEIGHT * 0.5;
    (0..num_control_points)
        .map(|i| {
            let t = i as f32 / (num_control_points - 1) as f32;
            egui::pos2(
                top_left.x + CELL_PADDING_X + t * (CELL_WIDTH - 2.0 * CELL_PADDING_X),
                center_y + AMPLITUDE * (t * NUM_PERIODS * std::f32::consts::TAU).sin(),
            )
        })
        .collect()
}

/// Top-left corner of the row for the given stroke width and renderer.
fn row_top_left(
    rect: egui::Rect,
    width_index: usize,
    column: usize,
    renderer: Renderer,
) -> egui::Pos2 {
    let renderer_index = Renderer::ALL
        .iter()
        .position(|r| *r == renderer)
        .unwrap_or_default();
    let row = width_index * Renderer::ALL.len() + renderer_index;
    egui::pos2(
        rect.left() + LABEL_WIDTH + column as f32 * CELL_WIDTH,
        rect.top() + HEADER_HEIGHT + row as f32 * ROW_HEIGHT,
    )
}

fn labels_ui(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    let font = egui::FontId::monospace(9.0);

    painter.text(
        egui::pos2(rect.left() + 2.0, rect.top() + 1.0),
        egui::Align2::LEFT_TOP,
        "a2c = alpha-to-coverage\nab = alpha blending\nep = epaint",
        egui::FontId::monospace(8.0),
        LABEL_COLOR,
    );

    for (column, num_control_points) in NUM_CONTROL_POINTS.iter().enumerate() {
        let top_left = row_top_left(rect, 0, column, Renderer::AlphaToCoverage);
        painter.text(
            egui::pos2(top_left.x + CELL_WIDTH * 0.5, rect.top() + 10.0),
            egui::Align2::CENTER_TOP,
            format!("n = {num_control_points}"),
            font.clone(),
            LABEL_COLOR,
        );
    }

    for (width_index, width) in STROKE_WIDTHS_UI.iter().enumerate() {
        for renderer in Renderer::ALL {
            let top_left = row_top_left(rect, width_index, 0, renderer);
            painter.text(
                egui::pos2(top_left.x - 4.0, top_left.y + ROW_HEIGHT * 0.5),
                egui::Align2::RIGHT_CENTER,
                format!("{width} {}", renderer.label()),
                font.clone(),
                LABEL_COLOR,
            );
        }
    }
}

fn epaint_lines_ui(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    for (width_index, &width) in STROKE_WIDTHS_UI.iter().enumerate() {
        for (column, &num_control_points) in NUM_CONTROL_POINTS.iter().enumerate() {
            let top_left = row_top_left(rect, width_index, column, Renderer::Epaint);
            painter.add(egui::Shape::line(
                wave(top_left, num_control_points),
                egui::Stroke::new(width, LINE_COLOR),
            ));
        }
    }
}

/// All the waves for one of the two `re_renderer` pipelines, as a single draw data.
fn line_draw_data(
    render_ctx: &RenderContext,
    rect: egui::Rect,
    renderer: Renderer,
) -> QueueableDrawData {
    let num_strips = STROKE_WIDTHS_UI.len() * NUM_CONTROL_POINTS.len();
    let num_vertices = NUM_CONTROL_POINTS.iter().sum::<usize>() * STROKE_WIDTHS_UI.len();

    let mut line_builder = LineDrawableBuilder::new(render_ctx);
    if renderer == Renderer::AlphaBlending {
        line_builder.enable_alpha_blending();
    }
    line_builder
        .reserve_strips(num_strips)
        .expect("failed to reserve line strips");
    line_builder
        .reserve_vertices(num_vertices)
        .expect("failed to reserve line vertices");

    {
        let mut batch = line_builder.batch("waves");
        for (width_index, &width) in STROKE_WIDTHS_UI.iter().enumerate() {
            for (column, &num_control_points) in NUM_CONTROL_POINTS.iter().enumerate() {
                let top_left = row_top_left(rect, width_index, column, renderer);
                let points = wave(top_left, num_control_points)
                    .into_iter()
                    .map(|p| glam::vec2(p.x, p.y));
                batch
                    .add_strip_2d(points)
                    .radius(Size::new_ui_points(width * 0.5))
                    .color(LINE_COLOR)
                    .flags(LINE_FLAGS);
            }
        }
    }

    line_builder
        .into_draw_data()
        .expect("failed to build line draw data")
        .into()
}

/// Paints one view per `re_renderer` pipeline, each with its own compositing mode.
///
/// The views cover the same rect and are cleared to transparent, so each one only
/// contributes the rows it drew.
fn re_renderer_lines_ui(ui: &egui::Ui, render_ctx: &RenderContext, rect: egui::Rect) {
    let pixels_per_point = ui.ctx().pixels_per_point();
    let resolution_in_pixel =
        re_viewer_context::gpu_bridge::viewport_resolution_in_pixels(rect, pixels_per_point);

    // The waves are built in absolute ui coordinates, so shift the camera to the rect corner.
    let view_from_world = macaw::IsoTransform::from_rotation_translation(
        glam::Quat::IDENTITY,
        glam::vec3(-rect.left(), -rect.top(), 0.0),
    );

    for (view_index, renderer) in Renderer::RE_RENDERER.into_iter().enumerate() {
        let target_config = TargetConfiguration {
            name: renderer.label().into(),
            resolution_in_pixel,
            view_from_world,
            projection_from_view: Projection::Orthographic {
                camera_mode: OrthographicCameraMode::TopLeftCornerAndExtendZ,
                vertical_world_size: rect.height(),
                far_plane_distance: 1000.0,
            },
            pixels_per_point,
            blend_with_background: renderer.blend_with_background(),
            ..Default::default()
        };

        let mut view_builder = ViewBuilder::new(
            render_ctx,
            target_config,
            re_renderer::ViewBuilderId::new(view_index as u64),
        )
        .expect("failed to create view builder");
        view_builder
            .queue_draw(render_ctx, line_draw_data(render_ctx, rect, renderer))
            .expect("failed to queue line draw data");

        ui.painter_at(rect)
            .add(re_viewer_context::gpu_bridge::new_renderer_callback(
                view_builder,
                rect,
                re_renderer::Rgba::TRANSPARENT,
            ));
    }
}

fn run_test(snapshot_name: &str, pixels_per_point: f32) {
    let test_context = TestContext::new();

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(total_size())
        .with_pixels_per_point(pixels_per_point)
        .build_ui(|ui| {
            let render_state = test_context.egui_render_state.lock();
            let render_state = render_state
                .as_ref()
                .expect("`setup_kittest_for_rendering_3d` should have created a render state");
            let mut egui_renderer = render_state.renderer.write();
            let render_ctx = egui_renderer
                .callback_resources
                .get_mut::<RenderContext>()
                .expect("no `re_renderer::RenderContext` in the egui render state");

            render_ctx.begin_frame();

            // The panel's inner margin would shift and clip the grid, so lay it out
            // against the whole viewport instead.
            let rect = ui.ctx().viewport_rect();
            ui.painter().rect_filled(rect, 0.0, Color32::BLACK);

            labels_ui(ui, rect);
            re_renderer_lines_ui(ui, render_ctx, rect);
            epaint_lines_ui(ui, rect);

            render_ctx.before_submit();
        });

    harness.snapshot(snapshot_name);
}

#[test]
fn test_lines_vs_epaint() {
    run_test("lines_vs_epaint", 1.0);
}

/// Same as [`test_lines_vs_epaint`], but on a high-dpi screen.
///
/// Sub-pixel handling is in physical pixels, so a 0.5 point stroke is a full physical
/// pixel wide here, while it is half a pixel wide in the 1x test.
#[test]
fn test_lines_vs_epaint_2x() {
    run_test("lines_vs_epaint_2x", 2.0);
}
