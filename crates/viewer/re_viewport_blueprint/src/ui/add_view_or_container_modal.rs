//! Modal for adding a new view of container to an existing target container.

use re_ui::UiExt as _;
use re_viewer_context::{
    blueprint_id_to_tile_id, icon_for_container_kind, ContainerId, RecommendedView, ViewerContext,
};

use crate::{ViewBlueprint, ViewportBlueprint};

#[derive(Default)]
pub struct AddViewOrContainerModal {
    target_container: Option<ContainerId>,
    modal_handler: re_ui::modal::ModalHandler,
}

impl AddViewOrContainerModal {
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
            egui_ctx,
            || {
                re_ui::modal::ModalWrapper::new("Add view or container")
                    .min_width(500.0)
                    .full_span_content(true)
                    .scrollable([false, true])
            },
            |ui, keep_open| modal_ui(ui, ctx, viewport, self.target_container, keep_open),
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
        let target_container_kind = target_container
            .and_then(|container_id| {
                viewport
                    .tree
                    .tiles
                    .get(blueprint_id_to_tile_id(&container_id))
            })
            .and_then(|tile| tile.container_kind());

        // We disallow creating "linear" containers (horizontal/vertical) inside containers of the same kind, because
        // it's not useful and is automatically simplified away.
        let disabled = Some(kind) == target_container_kind
            && matches!(
                kind,
                egui_tiles::ContainerKind::Horizontal | egui_tiles::ContainerKind::Vertical
            );

        let resp = ui
            .add_enabled_ui(!disabled, |ui| {
                row_ui(ui, icon_for_container_kind(&kind), title, subtitle)
            })
            .inner
            .on_disabled_hover_text(format!(
                "Nested {title} containers in containers of the same type are disallowed and automatically simplified \
                away as they are not useful."
            ));

        if resp.clicked() {
            viewport.add_container(kind, target_container);
            viewport.mark_user_interaction(ctx);
            *keep_open = false;
        }
    }

    ui.full_span_separator();

    // view of any kind
    for view in ctx
        .view_class_registry
        .iter_registry()
        .map(|entry| ViewBlueprint::new(entry.identifier, RecommendedView::root()))
    {
        let icon = view.class(ctx.view_class_registry).icon();
        let title = view.class(ctx.view_class_registry).display_name();
        let subtitle = format!("Create a new view to display {title} content.");

        if row_ui(ui, icon, title, &subtitle).clicked() {
            viewport.add_views(std::iter::once(view), target_container, None);
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
                fill: re_ui::design_tokens().thumbnail_background_color(),
                ..Default::default()
            }
            .show(ui, thumbnail_content);

            ui.vertical(|ui| {
                ui.strong(title);
                ui.add_space(-5.0);
                ui.add(egui::Label::new(subtitle).wrap_mode(egui::TextWrapMode::Extend));
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
