use egui::containers::menu::MenuState;
use egui::emath::{TSTransform, easing};
use egui::{AreaState, LayerId, Order, Pos2};

const INITIAL_SCALE: f32 = 0.95;

/// Adds an opening scale animation to egui popups.
///
/// Popups are separate foreground areas, and their placement pivot is the natural origin for the
/// animation.
pub struct PopupAnimationPlugin;

impl egui::Plugin for PopupAnimationPlugin {
    fn debug_name(&self) -> &'static str {
        "popup_animation"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        if !animations_enabled(ctx) {
            return;
        }

        let foreground_layers = ctx.memory(|memory| {
            memory
                .layer_ids()
                .filter(|layer_id| layer_id.order == Order::Foreground)
                .collect::<Vec<_>>()
        });

        for layer_id in foreground_layers {
            if !is_popup_layer(ctx, layer_id) || !is_layer_painted(ctx, layer_id) {
                continue;
            }

            let Some(area_state) = AreaState::load(ctx, layer_id.id) else {
                continue;
            };
            let Some(opened_at) = area_state.last_became_visible_at else {
                continue;
            };

            let scale = scale_for_opened_at(ctx, opened_at);
            if scale < 1.0 {
                let origin = area_state
                    .pivot_pos
                    .unwrap_or_else(|| area_state.rect().left_top());
                ctx.transform_layer_shapes_after_rounding(layer_id, scale_transform(origin, scale));
            }
        }
    }
}

fn is_popup_layer(ctx: &egui::Context, layer_id: LayerId) -> bool {
    // `Popup::show` marks its area as shown through `MenuState`, including popups whose open state
    // is not stored in egui memory, such as submenus.
    ctx.data(|data| {
        data.get_temp::<MenuState>(layer_id.id.with(MenuState::ID))
            .is_some()
    })
}

fn is_layer_painted(ctx: &egui::Context, layer_id: LayerId) -> bool {
    ctx.graphics(|graphics| {
        graphics
            .get(layer_id)
            .is_some_and(|paint_list| !paint_list.is_empty())
    })
}

/// Scale of an opening surface, from `INITIAL_SCALE` to 1. Requests a repaint until done.
pub fn scale_for_opened_at(ctx: &egui::Context, opened_at: f64) -> f32 {
    let animation_time = ctx.global_style().animation_time;
    if animation_time <= 0.0 {
        return 1.0;
    }

    let age = ctx.input(|input| (input.time - opened_at) as f32 + input.predicted_dt / 2.0);
    let progress = (age / animation_time).clamp(0.0, 1.0);
    let scale = egui::lerp(INITIAL_SCALE..=1.0, easing::cubic_out(progress));
    if scale < 1.0 {
        ctx.request_repaint();
    }
    scale
}

/// Transform that scales by `scale` around `origin`.
pub fn scale_transform(origin: Pos2, scale: f32) -> TSTransform {
    TSTransform::from_translation(origin.to_vec2())
        * TSTransform::from_scaling(scale)
        * TSTransform::from_translation(-origin.to_vec2())
}

fn animations_enabled(ctx: &egui::Context) -> bool {
    ctx.global_style().animation_time > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const POPUP_POS: Pos2 = egui::pos2(100.0, 100.0);
    const PLAIN_AREA_POS: Pos2 = egui::pos2(250.0, 250.0);

    /// After-rounding scale of each rect painted by [`render_floating_uis`], or `None` if it
    /// wasn't painted.
    struct FloatingUiScales {
        popup: Option<f32>,
        plain_area: Option<f32>,
    }

    fn render_floating_uis(ctx: &egui::Context, time: f64) -> FloatingUiScales {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 400.0),
                )),
                ..Default::default()
            },
            |ui| {
                let ctx = ui.ctx();
                egui::Popup::new(
                    egui::Id::unique("animated_popup"),
                    ctx.clone(),
                    POPUP_POS,
                    egui::LayerId::background(),
                )
                .frame(egui::Frame::NONE)
                .show(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(100.0, 100.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, egui::Color32::RED);
                });

                egui::Area::new(egui::Id::unique("plain_foreground_area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(PLAIN_AREA_POS)
                    .show(ctx, |ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(50.0, 50.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLUE);
                    });
            },
        );
        output.textures_delta.clear();

        let scale_of = |pos| {
            output
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Rect(rect) if rect.rect.min == pos => {
                        Some(clipped.transform_after_tessellation.scaling)
                    }
                    _ => None,
                })
        };
        FloatingUiScales {
            popup: scale_of(POPUP_POS),
            plain_area: scale_of(PLAIN_AREA_POS),
        }
    }

    #[test]
    fn plugin_only_scales_new_popups() {
        let ctx = egui::Context::default();
        ctx.add_plugin(PopupAnimationPlugin);
        ctx.all_styles_mut(|style| style.animation_time = 0.2);

        render_floating_uis(&ctx, 0.0);
        let opening = render_floating_uis(&ctx, 0.016);
        assert!(opening.popup.expect("the popup should be painted") < 1.0);
        assert_eq!(opening.plain_area, Some(1.0));

        let popup_layer = LayerId::new(Order::Foreground, egui::Id::unique("animated_popup"));
        assert_eq!(
            ctx.layer_transform_to_global(popup_layer),
            None,
            "the animation must not move the popup's pixel grid or input coordinates"
        );

        assert_eq!(render_floating_uis(&ctx, 1.0).popup, Some(1.0));
    }
}
