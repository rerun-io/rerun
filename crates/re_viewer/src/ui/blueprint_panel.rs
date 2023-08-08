use re_viewer_context::ViewerContext;
use re_viewport::{SpaceInfoCollection, ViewportBlueprint};

/// Show the left-handle panel based on the current [`ViewportBlueprint`]
pub fn blueprint_panel_ui(
    blueprint: &mut ViewportBlueprint<'_>,
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    spaces_info: &SpaceInfoCollection,
    expanded: bool,
) {
    let screen_width = ui.ctx().screen_rect().width();

    let panel = egui::SidePanel::left("blueprint_panel")
        .resizable(true)
        .frame(egui::Frame {
            fill: ui.visuals().panel_fill,
            ..Default::default()
        })
        .min_width(120.0)
        .default_width((0.35 * screen_width).min(200.0).round());

    panel.show_animated_inside(ui, expanded, |ui: &mut egui::Ui| {
        // no need to extend `ui.max_rect()` as the enclosing frame doesn't have margins
        ui.set_clip_rect(ui.max_rect());

        title_bar_ui(blueprint, ctx, ui, spaces_info);

        egui::Frame {
            inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
            ..Default::default()
        }
        .show(ui, |ui| {
            blueprint.tree_ui(ctx, ui);
        });
    });
}

fn title_bar_ui(
    blueprint: &mut ViewportBlueprint<'_>,
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    spaces_info: &SpaceInfoCollection,
) {
    egui::TopBottomPanel::top("blueprint_panel_title_bar")
        .exact_height(re_ui::ReUi::title_bar_height())
        .frame(egui::Frame {
            inner_margin: egui::Margin::symmetric(re_ui::ReUi::view_padding(), 0.0),
            ..Default::default()
        })
        .show_inside(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.strong("Blueprint")
                    .on_hover_text("The Blueprint is where you can configure the Rerun Viewer");

                ui.allocate_ui_with_layout(
                    ui.available_size_before_wrap(),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        blueprint.add_new_spaceview_button_ui(ctx, ui, spaces_info);
                        reset_button_ui(blueprint, ctx, ui, spaces_info);
                    },
                );
            });
        });
}

fn reset_button_ui(
    blueprint: &mut ViewportBlueprint<'_>,
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    spaces_info: &SpaceInfoCollection,
) {
    if ctx
        .re_ui
        .small_icon_button(ui, &re_ui::icons::RESET)
        .on_hover_text("Re-populate Viewport with automatically chosen Space Views")
        .clicked()
    {
        blueprint.reset(ctx, spaces_info);
    }
}
