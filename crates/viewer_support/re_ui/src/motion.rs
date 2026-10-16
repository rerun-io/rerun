use egui::containers::menu::MenuState;
use egui::emath::{TSTransform, easing};
use egui::{AreaState, LayerId, Order, Pos2};

const INITIAL_SCALE: f32 = 0.95;

/// Adds an opening scale animation to egui popups.
///
/// Popups are separate foreground areas, and their placement pivot is the natural origin for the
/// animation.
#[derive(Default)]
pub(crate) struct PopupAnimationPlugin {
    transformed_layers: Vec<LayerId>,
}

impl egui::Plugin for PopupAnimationPlugin {
    fn debug_name(&self) -> &'static str {
        "popup_animation"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        if !animations_enabled(ctx) {
            self.clear_transforms(ctx);
            return;
        }

        let foreground_layers = ctx.memory(|memory| {
            memory
                .layer_ids()
                .filter(|layer_id| layer_id.order == Order::Foreground)
                .collect::<Vec<_>>()
        });

        let mut transformed_layers = Vec::new();
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
                ctx.set_transform_layer(layer_id, scale_transform(origin, scale));
                transformed_layers.push(layer_id);
            }
        }

        for layer_id in &self.transformed_layers {
            if !transformed_layers.contains(layer_id) {
                ctx.set_transform_layer(*layer_id, TSTransform::IDENTITY);
            }
        }
        self.transformed_layers = transformed_layers;
    }
}

impl PopupAnimationPlugin {
    fn clear_transforms(&mut self, ctx: &egui::Context) {
        for layer_id in self.transformed_layers.drain(..) {
            ctx.set_transform_layer(layer_id, TSTransform::IDENTITY);
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

pub(crate) fn scale_for_opened_at(ctx: &egui::Context, opened_at: f64) -> f32 {
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

pub(crate) fn scale_transform(origin: Pos2, scale: f32) -> TSTransform {
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

    fn render_floating_uis(ctx: &egui::Context, time: f64, show_popup: bool) {
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
                if show_popup {
                    egui::Popup::new(
                        egui::Id::unique("animated_popup"),
                        ctx.clone(),
                        egui::pos2(100.0, 100.0),
                        egui::LayerId::background(),
                    )
                    .frame(egui::Frame::NONE)
                    .show(|ui| {
                        ui.allocate_exact_size(egui::vec2(100.0, 100.0), egui::Sense::hover());
                    });
                }

                egui::Area::new(egui::Id::unique("plain_foreground_area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(250.0, 250.0))
                    .show(ctx, |ui| {
                        ui.allocate_exact_size(egui::vec2(50.0, 50.0), egui::Sense::hover());
                    });
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn plugin_only_scales_new_popups() {
        let ctx = egui::Context::default();
        ctx.add_plugin(PopupAnimationPlugin::default());
        ctx.all_styles_mut(|style| style.animation_time = 0.2);

        render_floating_uis(&ctx, 0.0, true);
        render_floating_uis(&ctx, 0.016, true);

        let popup_layer = LayerId::new(Order::Foreground, egui::Id::unique("animated_popup"));
        let popup_transform = ctx
            .layer_transform_to_global(popup_layer)
            .expect("the opening popup should have an input-aware transform");
        assert!(popup_transform.scaling < 1.0);

        let plain_area_layer =
            LayerId::new(Order::Foreground, egui::Id::unique("plain_foreground_area"));
        assert_eq!(ctx.layer_transform_to_global(plain_area_layer), None);

        render_floating_uis(&ctx, 0.032, false);
        assert_eq!(ctx.layer_transform_to_global(popup_layer), None);
    }
}
