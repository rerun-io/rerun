//! Modal for adding a new space view of container to an existing target container.

use itertools::Itertools;

use crate::{icon_for_container_kind, ViewportBlueprint};
use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::{DataQueryBlueprint, SpaceViewBlueprint};
use re_ui::ReUi;
use re_viewer_context::{ContainerId, ViewerContext};

#[derive(Default)]
pub struct AddSpaceViewOrContainerModal {
    target_container: Option<ContainerId>,
    modal_handler: re_ui::modal::ModalHandler,
}

impl AddSpaceViewOrContainerModal {
    pub(crate) fn open(&mut self, target_container: ContainerId) {
        self.target_container = Some(target_container);
        self.modal_handler.open();
    }

    pub(crate) fn ui(
        &mut self,
        egui_ctx: &egui::Context,
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
    ) {
        self.modal_handler.ui(
            ctx.re_ui,
            egui_ctx,
            || {
                re_ui::modal::Modal::new("Add Space View or Container")
                    .min_width(500.0)
                    .full_span_content(true)
            },
            |_, ui, keep_open| modal_ui(ui, ctx, viewport, self.target_container, keep_open),
        );
    }
}

fn modal_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    target_container: Option<ContainerId>,
    keep_open: &mut bool,
) {
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
        if row_ui(ui, icon_for_container_kind(&kind), title, subtitle).clicked() {
            viewport.add_container(kind, target_container);
            viewport.mark_user_interaction(ctx);
            *keep_open = false;
        }
    }

    ReUi::full_span_separator(ui);

    // space view of any kind
    for space_view in ctx
        .space_view_class_registry
        .iter_registry()
        .sorted_by_key(|entry| entry.class.display_name())
        .map(|entry| {
            SpaceViewBlueprint::new(
                entry.class.identifier(),
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
            viewport.add_space_views(std::iter::once(space_view), ctx, target_container);
            viewport.mark_user_interaction(ctx);
            *keep_open = false;
        }
    }
}

/// Draw a single row.
///
/// Each row must ensure its own spacing. Here is the geometry we target:
/// ```text
///          available_width (defined by `Modal`)
///      │◀───────────────────────────────────────▶│
///      │                                         │
/// ┌───────────────────────────────────────────────────┐──▲
/// │                                                   │  │  row_space/2
/// │    ╔══════╦══════════════════════════════════╗────│──▼▲
/// │    ║      ║                                  ║    │   │
/// │    ║ Icon ║  Title and Subtitles             ║    │   │ row_height
/// │    ║      ║                                  ║    │   │
/// │    ╚══════╩══════════════════════════════════╝────│──▲▼
/// │                                                   │  │  row_space/2
/// └───────────────────────────────────────────────────┘──▼
/// │                                                   │
/// │◀─────────────────────────────────────────────────▶│
///                       clip_rect
/// ```
fn row_ui(ui: &mut egui::Ui, icon: &re_ui::Icon, title: &str, subtitle: &str) -> egui::Response {
    //TODO(ab): use design tokens
    let row_space = 14.0;
    let row_height = 42.0;
    let icon_size = egui::vec2(18.0, 18.0);
    let thumbnail_rounding = 6.0;
    let thumbnail_width = 62.0;

    let top_left_corner = ui.cursor().min;

    ui.add_space(row_space / 2.0);

    let resp = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(14.0, 10.0);

            // placeholder for the hover background
            let background_frame = ui.painter().add(egui::Shape::Noop);

            let thumbnail_content = |ui: &mut egui::Ui| {
                let (rect, _) = ui.allocate_exact_size(icon_size, egui::Sense::hover());
                icon.as_image()
                    .tint(ui.visuals().widgets.active.fg_stroke.color)
                    .paint_at(ui, rect);
            };

            egui::Frame {
                inner_margin: egui::Margin::symmetric(
                    (thumbnail_width - icon_size.x) / 2.0,
                    (row_height - icon_size.y) / 2.0,
                ), // should be 62x42 when combined with icon size
                rounding: egui::Rounding::same(thumbnail_rounding),
                fill: egui::Color32::from_gray(50),
                ..Default::default()
            }
            .show(ui, thumbnail_content);

            ui.vertical(|ui| {
                ui.strong(title);
                ui.add_space(-5.0);
                ui.add(egui::Label::new(subtitle).wrap(false));
            });

            let right_coord = ui.cursor().max.x;

            // interact with the entire row
            let interact_rect = egui::Rect::from_min_max(
                top_left_corner,
                egui::pos2(right_coord, top_left_corner.y + row_height + row_space),
            );

            let response =
                ui.interact(interact_rect, title.to_owned().into(), egui::Sense::click());

            if response.hovered() {
                let clip_rect = ui.clip_rect();

                let bg_rect = interact_rect
                    .with_min_x(clip_rect.min.x)
                    .with_max_x(clip_rect.max.x);

                ui.painter().set(
                    background_frame,
                    egui::Shape::rect_filled(
                        bg_rect,
                        0.0,
                        ui.visuals().widgets.hovered.weak_bg_fill,
                    ),
                );
            }

            response
        })
        .inner;

    ui.add_space(row_space / 2.0);

    resp
}
