//! Modal for adding a new space view of container to an existing target container.

use itertools::Itertools;

use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::DataQueryBlueprint;
use re_viewer_context::ViewerContext;
use re_viewport::{SpaceViewBlueprint, Viewport};

#[derive(Default)]
pub struct AddSpaceViewOrContainerModal {
    target_container: Option<egui_tiles::TileId>,
    modal_handler: re_ui::modal::ModalHandler,
}

impl AddSpaceViewOrContainerModal {
    pub fn open(&mut self, target_container: egui_tiles::TileId) {
        self.target_container = Some(target_container);
        self.modal_handler.open();
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &ViewerContext<'_>,
        viewport: &mut Viewport<'_, '_>,
    ) {
        self.modal_handler.ui(
            ctx.re_ui,
            ui,
            || re_ui::modal::Modal::new("Add Space View or Container"),
            |_, ui, _| modal_ui(ui, ctx, viewport, self.target_container),
        );
    }
}

fn modal_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &mut Viewport<'_, '_>,
    target_container: Option<egui_tiles::TileId>,
) {
    ui.spacing_mut().item_spacing = egui::vec2(14.0, 10.0);

    let container_data = [
        (
            "Tabs",
            "Create a new tabbed container.",
            egui_tiles::ContainerKind::Tabs,
        ),
        (
            "Horizontal",
            "Create a new horizontal container.",
            egui_tiles::ContainerKind::Horizontal,
        ),
        (
            "Vertical",
            "Create a new vertical container.",
            egui_tiles::ContainerKind::Vertical,
        ),
        (
            "Grid",
            "Create a new grid container.",
            egui_tiles::ContainerKind::Grid,
        ),
    ];

    for (title, subtitle, kind) in container_data {
        if row_ui(ui, &re_ui::icons::CONTAINER, title, subtitle).clicked() {
            viewport.blueprint.add_container(kind, target_container);
            viewport.blueprint.mark_user_interaction(ctx);
        }
    }

    ui.separator();

    // space view of any kind
    for space_view in ctx
        .space_view_class_registry
        .iter_registry()
        .sorted_by_key(|entry| entry.class.display_name())
        .map(|entry| {
            SpaceViewBlueprint::new(
                entry.class.identifier(),
                &format!("empty {}", entry.class.display_name()),
                &EntityPath::root(),
                DataQueryBlueprint::new(entry.class.identifier(), EntityPathFilter::default()),
            )
        })
    {
        let icon = space_view.class(ctx.space_view_class_registry).icon();
        let title = space_view
            .class(ctx.space_view_class_registry)
            .display_name();
        let subtitle = format!("Create a new Space View to display {title} content.");

        if row_ui(ui, icon, title, &subtitle).clicked() {
            viewport.blueprint.add_space_views(
                std::iter::once(space_view),
                ctx,
                target_container,
                false,
            );
            viewport.blueprint.mark_user_interaction(ctx);
        }
    }
}

fn row_ui(ui: &mut egui::Ui, icon: &re_ui::Icon, title: &str, subtitle: &str) -> egui::Response {
    ui.horizontal(|ui| {
        //TODO(ab): move this to re_ui
        //TODO(ab): use design token
        let icon_size = egui::vec2(18.0, 18.0);
        egui::Frame {
            inner_margin: egui::Margin::symmetric(
                (62. - icon_size.x) / 2.0,
                (42. - icon_size.y) / 2.0,
            ), // should be 62x42 when combined with icon size
            rounding: egui::Rounding::same(6.0),
            fill: egui::Color32::from_gray(50),
            ..Default::default()
        }
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(icon_size, egui::Sense::hover());
            icon.as_image()
                .tint(ui.visuals().widgets.active.fg_stroke.color)
                .paint_at(ui, rect);
        });

        ui.vertical(|ui| {
            ui.strong(title);
            ui.add_space(-5.0);

            ui.add(egui::Label::new(subtitle).wrap(false));
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::ImageButton::new(
                    re_ui::icons::ADD_BIG
                        .as_image()
                        .fit_to_exact_size(egui::vec2(24.0, 24.0)),
                )
                .tint(ui.visuals().widgets.inactive.fg_stroke.color),
            )
        })
        .inner
    })
    .inner
}
