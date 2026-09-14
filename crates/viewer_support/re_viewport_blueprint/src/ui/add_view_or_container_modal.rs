//! Modal for adding a new view of container to an existing target container.

use re_ui::UiExt as _;
use re_viewer_context::{
    ContainerId, ViewerContext, blueprint_id_to_tile_id, icon_for_container_kind,
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
            |ui| modal_ui(ui, ctx, viewport, self.target_container),
        );
    }
}

fn modal_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    target_container: Option<ContainerId>,
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
                row_ui(ui, icon_for_container_kind(&kind), title, subtitle, false)
            })
            .inner
            .on_disabled_hover_text(format!(
                "Nested {title} containers in containers of the same type are disallowed and automatically simplified \
                away as they are not useful."
            ));

        if resp.clicked() {
            viewport.add_container(kind, target_container);
            viewport.mark_user_interaction(ctx);
            ui.close();
        }
    }

    ui.full_span_separator();

    // Split views into stable / experimental groups. Experimental views go into a separate
    // section at the bottom of the modal with a warning icon — they're fully functional, just
    // marked clearly as in-flux.
    let (stable_views, experimental_views): (Vec<_>, Vec<_>) = ctx
        .view_class_registry()
        .iter_registry()
        .map(|entry| ViewBlueprint::new_with_root_wildcard(entry.identifier))
        .partition(|view| !view.class(ctx.view_class_registry()).is_experimental());

    let add_view_row = |ui: &mut egui::Ui, view: ViewBlueprint, is_experimental: bool| {
        let icon = view.class(ctx.view_class_registry()).icon();
        let title = view.class(ctx.view_class_registry()).display_name();
        let subtitle = format!("Create a new view to display {title} content.");

        if row_ui(ui, icon, title, &subtitle, is_experimental).clicked() {
            viewport.add_views(std::iter::once(view), target_container, None);
            viewport.mark_user_interaction(ctx);
            ui.close();
        }
    };

    for view in stable_views {
        add_view_row(ui, view, false);
    }

    if !experimental_views.is_empty() {
        ui.full_span_separator();
        for view in experimental_views {
            add_view_row(ui, view, true);
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
fn row_ui(
    ui: &mut egui::Ui,
    icon: &re_ui::Icon,
    title: &str,
    subtitle: &str,
    is_experimental: bool,
) -> egui::Response {
    //TODO(ab): use design tokens
    let row_space = 14.0;
    let row_height = 42.0;
    let icon_size = egui::vec2(18.0, 18.0);
    let thumbnail_rounding = 6;
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
                    ((thumbnail_width - icon_size.x) / 2.0) as i8,
                    ((row_height - icon_size.y) / 2.0) as i8,
                ), // should be 62x42 when combined with icon size
                corner_radius: egui::CornerRadius::same(thumbnail_rounding),
                fill: ui.tokens().thumbnail_background_color,
                ..Default::default()
            }
            .show(ui, thumbnail_content);

            ui.vertical(|ui| {
                ui.strong(title);
                ui.add_space(-5.0);
                if is_experimental {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add(
                            re_ui::icons::WARNING
                                .as_image()
                                .tint(ui.tokens().alert_info.icon),
                        );
                        ui.add(
                            egui::Label::new(format!("Experimental: {subtitle}").as_str())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
                } else {
                    ui.add(egui::Label::new(subtitle).wrap_mode(egui::TextWrapMode::Extend));
                }
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
