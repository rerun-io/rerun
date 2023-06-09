use re_data_store::StoreDb;
use re_viewer_context::{Item, ViewerContext};
use re_viewport::{SpaceInfoCollection, Viewport};

/// Defines the layout of the whole Viewer (or will, eventually).
#[derive(Clone)]
pub struct Blueprint<'a> {
    pub blueprint: Option<&'a StoreDb>,

    pub viewport: Viewport,
}

impl<'a> Blueprint<'a> {
    /// Create a [`Blueprint`] with appropriate defaults.
    pub fn new(blueprint: Option<&'a StoreDb>) -> Self {
        Self {
            blueprint,
            viewport: Default::default(),
        }
    }

    pub fn show_panel(
        &mut self,
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
            self.title_bar_ui(ctx, ui, spaces_info);

            egui::Frame {
                inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                ..Default::default()
            }
            .show(ui, |ui| {
                self.viewport.tree_ui(ctx, ui);
            });
        });
    }

    fn title_bar_ui(
        &mut self,
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
                    ui.strong("Blueprint").on_hover_text(
                        "The Blueprint is where you can configure the Rerun Viewer.",
                    );

                    ui.allocate_ui_with_layout(
                        ui.available_size_before_wrap(),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            self.viewport
                                .add_new_spaceview_button_ui(ctx, ui, spaces_info);
                            self.reset_button_ui(ctx, ui, spaces_info);
                        },
                    );
                });
            });
    }

    fn reset_button_ui(
        &mut self,
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
            self.viewport = Viewport::new(ctx, spaces_info);
        }
    }

    /// If `false`, the item is referring to data that is not present in this blueprint.
    pub fn is_item_valid(&self, item: &Item) -> bool {
        match item {
            Item::ComponentPath(_) => true,
            Item::InstancePath(space_view_id, _) => space_view_id
                .map(|space_view_id| self.viewport.space_view(&space_view_id).is_some())
                .unwrap_or(true),
            Item::SpaceView(space_view_id) => self.viewport.space_view(space_view_id).is_some(),
            Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
                if let Some(space_view) = self.viewport.space_view(space_view_id) {
                    space_view
                        .data_blueprint
                        .group(*data_blueprint_group_handle)
                        .is_some()
                } else {
                    false
                }
            }
        }
    }
}
