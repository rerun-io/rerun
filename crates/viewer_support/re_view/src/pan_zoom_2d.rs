use egui::emath::RectTransform;
use egui::{Rect, Vec2};
use re_sdk_types::blueprint::archetypes::VisualBounds2D;
use re_sdk_types::blueprint::components;
use re_viewer_context::ViewerContext;
use re_viewport_blueprint::ViewProperty;

/// Cap on how large the 2D visible area is allowed to grow, measured per-axis as a multiple
/// of the matching content extent.
///
/// Applied to zoom-out only: zooming stops as soon as either width reaches `content_size.x * factor`
/// or height reaches `content_size.y * factor`, so the aspect ratio of the bounds is kept.
/// Bounds that are already past the limit are never pulled back in; zoom-in is never restricted.
const MAX_ZOOM_OUT_FACTOR: f32 = 5.0;

/// How [`pan_zoom_visual_bounds_2d`] maps the visual bounds onto the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualBoundsFit {
    /// Grow the bounds along one axis so they fill the view at a uniform scale.
    Letterbox,

    /// Stretch the bounds over the view, scaling each axis independently.
    Stretch,
}

/// Pans and zooms the [`VisualBounds2D`] of a view from this frame's input on `response`.
///
/// Dragging and scrolling pan, the zoom gesture zooms around the pointer,
/// and a double-click resets the bounds to the value in the default blueprint.
/// Changes are written to `bounds_property`, so they take effect next frame.
///
/// `bounds` is the value the caller resolved for this frame, from the blueprint or a fallback.
/// `fit` says how `bounds` is mapped onto `response.rect`.
///
/// `content_size` is the scene size of what the view shows, which caps how far it can zoom out.
///
/// Returns the transform from scene coordinates to ui coordinates for `bounds`.
pub fn pan_zoom_visual_bounds_2d(
    ctx: &ViewerContext<'_>,
    response: &egui::Response,
    bounds_property: &ViewProperty,
    bounds: components::VisualBounds2D,
    fit: VisualBoundsFit,
    content_size: Option<Vec2>,
) -> RectTransform {
    let bounds_rect = Rect::from(bounds);
    let shown_bounds = match fit {
        VisualBoundsFit::Letterbox => letterboxed(bounds_rect, response.rect),
        VisualBoundsFit::Stretch => bounds_rect,
    };
    let ui_from_scene = RectTransform::from_to(shown_bounds, response.rect);

    let updated_bounds: components::VisualBounds2D =
        pan_and_zoom(response, bounds_rect, &ui_from_scene, content_size).into();

    if response.double_clicked() {
        bounds_property.reset_blueprint_component(ctx, VisualBounds2D::descriptor_range());
    } else if bounds != updated_bounds {
        bounds_property.save_blueprint_component(
            ctx,
            &VisualBounds2D::descriptor_range(),
            &updated_bounds,
        );
    }

    ui_from_scene
}

/// Grows `bounds` along one axis so it has the aspect ratio of `ui_rect`, keeping it centered.
fn letterboxed(bounds: Rect, ui_rect: Rect) -> Rect {
    let mut letterboxed = bounds;

    let ui_from_scene = RectTransform::from_to(bounds, ui_rect);
    let scale_aspect = ui_from_scene.scale().x / ui_from_scene.scale().y;
    if scale_aspect < 1.0 {
        let add = bounds.height() * (1.0 / scale_aspect - 1.0);
        letterboxed.min.y -= 0.5 * add;
        letterboxed.max.y += 0.5 * add;
    } else {
        let add = bounds.width() * (scale_aspect - 1.0);
        letterboxed.min.x -= 0.5 * add;
        letterboxed.max.x += 0.5 * add;
    }

    letterboxed
}

/// Applies this frame's pan and zoom input to `bounds`.
///
/// `ui_from_scene` is the transform `bounds` is currently shown with.
fn pan_and_zoom(
    response: &egui::Response,
    mut bounds: Rect,
    ui_from_scene: &RectTransform,
    content_size: Option<Vec2>,
) -> Rect {
    let mut pan_delta_in_ui = response.drag_delta();
    if response.hovered() {
        pan_delta_in_ui += response.ctx.input(|i| i.smooth_scroll_delta);
    }
    if pan_delta_in_ui != Vec2::ZERO {
        bounds = bounds.translate(-pan_delta_in_ui / ui_from_scene.scale());
    }

    if response.hovered() {
        let zoom_delta = response.ctx.input(|i| i.zoom_delta_2d());

        if zoom_delta != Vec2::splat(1.0) {
            let zoom_center_in_ui = response
                .hover_pos()
                .unwrap_or_else(|| response.rect.center());
            let zoom_center_in_scene = ui_from_scene
                .inverse()
                .transform_pos(zoom_center_in_ui)
                .to_vec2();
            let candidate = scale_rect(
                bounds.translate(-zoom_center_in_scene),
                Vec2::splat(1.0) / zoom_delta,
            )
            .translate(zoom_center_in_scene);

            bounds = clamp_zoom_out(bounds, candidate, zoom_center_in_scene, content_size);
        }
    }

    bounds
}

fn scale_rect(rect: Rect, factor: Vec2) -> Rect {
    Rect::from_min_max(
        (factor * rect.min.to_vec2()).to_pos2(),
        (factor * rect.max.to_vec2()).to_pos2(),
    )
}

/// Caps zoom-out at [`MAX_ZOOM_OUT_FACTOR`] times `content_size`.
///
/// Both axes are scaled back by the same factor, so the aspect ratio of `candidate` is kept.
///
/// If `current` is already past the cap (e.g. right after loading), its size is the cap instead, so there is no snap-back.
/// Zooming in is never restricted.
fn clamp_zoom_out(
    current: Rect,
    candidate: Rect,
    zoom_center: Vec2,
    content_size: Option<Vec2>,
) -> Rect {
    // `1.0e17` fallback is chosen with generous margin of an observed crash due to infinity.
    let fallback = Vec2::splat(1.0e17);

    let max_size = content_size
        .map(|size| size * MAX_ZOOM_OUT_FACTOR)
        .filter(|max| max.x.is_finite() && max.x > 0.0 && max.y.is_finite() && max.y > 0.0)
        .unwrap_or(fallback)
        .max(current.size());

    let shrink = (max_size / candidate.size()).min_elem();
    if shrink >= 1.0 {
        candidate
    } else {
        scale_rect(candidate.translate(-zoom_center), Vec2::splat(shrink)).translate(zoom_center)
    }
}

#[cfg(test)]
mod tests {
    use egui::{Rect, Vec2, pos2, vec2};

    use super::{clamp_zoom_out, letterboxed};

    #[test]
    fn letterbox_wide_ui() {
        let bounds = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let ui_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0));
        assert_eq!(
            letterboxed(bounds, ui_rect),
            Rect::from_min_max(pos2(-5.0, 0.0), pos2(15.0, 10.0))
        );
    }

    #[test]
    fn letterbox_tall_ui() {
        let bounds = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let ui_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 200.0));
        assert_eq!(
            letterboxed(bounds, ui_rect),
            Rect::from_min_max(pos2(0.0, -5.0), pos2(10.0, 15.0))
        );
    }

    fn assert_approx_eq(a: Vec2, b: Vec2) {
        assert!((a - b).length() < 1e-3, "{a:?} != {b:?}");
    }

    #[test]
    fn zoom_out_is_capped_by_content_size() {
        let current = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let candidate = Rect::from_min_max(pos2(-100.0, -100.0), pos2(110.0, 110.0));
        let clamped = clamp_zoom_out(current, candidate, vec2(5.0, 5.0), Some(vec2(10.0, 10.0)));
        assert_approx_eq(clamped.size(), vec2(50.0, 50.0));
        assert_approx_eq(clamped.center().to_vec2(), vec2(5.0, 5.0));
    }

    #[test]
    fn zoom_out_stops_when_first_axis_hits_cap() {
        // Width reaches its cap of 50 before height reaches its cap of 50.
        let current = Rect::from_min_max(pos2(0.0, 0.0), pos2(40.0, 10.0));
        let candidate = Rect::from_min_max(pos2(0.0, 0.0), pos2(80.0, 20.0));
        let clamped = clamp_zoom_out(current, candidate, vec2(0.0, 0.0), Some(vec2(10.0, 10.0)));
        assert_approx_eq(clamped.size(), vec2(50.0, 12.5));

        let again = clamp_zoom_out(clamped, candidate, vec2(0.0, 0.0), Some(vec2(10.0, 10.0)));
        assert_approx_eq(again.size(), vec2(50.0, 12.5));
    }

    #[test]
    fn zoom_in_is_never_capped() {
        let current = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let candidate = Rect::from_min_max(pos2(4.0, 4.0), pos2(6.0, 6.0));
        assert_eq!(
            clamp_zoom_out(current, candidate, vec2(5.0, 5.0), Some(vec2(10.0, 10.0))),
            candidate
        );
    }

    #[test]
    fn already_past_cap_is_kept() {
        let current = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        let candidate = Rect::from_min_max(pos2(0.0, 0.0), pos2(120.0, 120.0));
        let clamped = clamp_zoom_out(current, candidate, vec2(0.0, 0.0), Some(vec2(1.0, 1.0)));
        assert_approx_eq(clamped.size(), vec2(100.0, 100.0));
    }
}
