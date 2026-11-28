//! Modal for adding a new view of container to an existing target container.

use egui::AtomExt as _;
use re_ui::{ButtonExt as _, UiExt as _};
use re_viewer_context::{
    ContainerId, ViewerContext, blueprint_id_to_tile_id, icon_for_container_kind,
};

use crate::{ViewBlueprint, ViewportBlueprint};

const NUM_COLUMNS: usize = 4;
const CARD_HEIGHT: f32 = 76.0;
const CARD_SPACING: f32 = 8.0;
const ICON_SIZE: egui::Vec2 = egui::Vec2::splat(20.0);
const ICON_TITLE_GAP: f32 = 10.0;
const TITLE_TAG_GAP: f32 = 4.0;

/// Text looks smaller than its bounding box, so centered content appears to sit too high.
const OPTICAL_TOP_PADDING: i8 = 2;

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
        ("Tabs", egui_tiles::ContainerKind::Tabs),
        ("Horizontal", egui_tiles::ContainerKind::Horizontal),
        ("Vertical", egui_tiles::ContainerKind::Vertical),
        ("Grid", egui_tiles::ContainerKind::Grid),
    ];

    let target_container_kind = target_container
        .and_then(|container_id| {
            viewport
                .tree
                .tiles
                .get(blueprint_id_to_tile_id(&container_id))
        })
        .and_then(|tile| tile.container_kind());

    card_grid_ui(ui, "containers", container_data, |ui, (title, kind)| {
        // We disallow creating "linear" containers (horizontal/vertical) inside containers of the same kind, because
        // it's not useful and is automatically simplified away.
        let enabled = match kind {
            egui_tiles::ContainerKind::Tabs | egui_tiles::ContainerKind::Grid => true,
            egui_tiles::ContainerKind::Horizontal | egui_tiles::ContainerKind::Vertical => {
                Some(kind) != target_container_kind
            }
        };

        let response = card_ui(ui, icon_for_container_kind(&kind), title, None, enabled)
            .on_disabled_hover_text(format!(
                "Nested {title} containers in containers of the same type are disallowed and automatically simplified \
                away as they are not useful."
            ));

        if response.clicked() {
            viewport.add_container(kind, target_container);
            viewport.mark_user_interaction(ctx);
            ui.close();
        }
    });

    ui.full_span_separator();

    let view_classes = ctx
        .view_class_registry()
        .iter_registry()
        .map(|entry| entry.identifier);

    card_grid_ui(ui, "views", view_classes, |ui, view_class| {
        let class = ctx.view_class_registry().get_class_or_log_error(view_class);
        let experimental = ctx
            .reflection()
            .views
            .get(&view_class)
            .is_some_and(|view| view.experimental);
        let tag = experimental.then(|| ui.experimental_tag_atom());

        if card_ui(ui, class.icon(), class.display_name(), tag, true).clicked() {
            let view = ViewBlueprint::new_for_user_request(ctx, view_class);
            viewport.add_views(std::iter::once(view), target_container, None);
            viewport.mark_user_interaction(ctx);
            ui.close();
        }
    });
}

/// Show one card per item, in a grid that fills the available width.
fn card_grid_ui<T>(
    ui: &mut egui::Ui,
    id_salt: &str,
    items: impl IntoIterator<Item = T>,
    mut add_card: impl FnMut(&mut egui::Ui, T),
) {
    let column_width =
        (ui.available_width() - (NUM_COLUMNS - 1) as f32 * CARD_SPACING) / NUM_COLUMNS as f32;

    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(0, ui.tokens().view_padding()))
        .show(ui, |ui| {
            egui::Grid::new(id_salt)
                .num_columns(NUM_COLUMNS)
                .min_col_width(column_width)
                .max_col_width(column_width)
                .min_row_height(CARD_HEIGHT)
                .spacing(egui::Vec2::splat(CARD_SPACING))
                .show(ui, |ui| {
                    for (i, item) in items.into_iter().enumerate() {
                        if 0 < i && i % NUM_COLUMNS == 0 {
                            ui.end_row();
                        }
                        add_card(ui, item);
                    }
                });
        });
}

/// Show a button that fills its grid cell, with an icon above a title.
///
/// A `tag`, such as [`re_ui::UiExt::experimental_tag_atom`], goes below the title.
fn card_ui(
    ui: &mut egui::Ui,
    icon: &re_ui::Icon,
    title: &str,
    tag: Option<egui::ContainerAtom<'static>>,
    enabled: bool,
) -> egui::Response {
    // The button only styles its top-level atoms, so the nested icon has to be tinted here.
    let icon = icon
        .as_image()
        .tint(re_ui::Variant::Secondary.visuals(ui.tokens()).text)
        .atom_size(ICON_SIZE);

    let mut label = egui::Atoms::new(title);
    if let Some(tag) = tag {
        label.push_right(tag);
    }
    let label = egui::ContainerAtom::new(label)
        .direction(egui::Direction::TopDown)
        .gap(TITLE_TAG_GAP);

    let content = egui::ContainerAtom::new((icon, label))
        .direction(egui::Direction::TopDown)
        .gap(ICON_TITLE_GAP)
        .wrap_mode(egui::TextWrapMode::Truncate)
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            top: OPTICAL_TOP_PADDING,
            ..Default::default()
        }));

    let button = egui::Button::new(content).secondary();

    ui.add_enabled_ui(enabled, |ui| {
        ui.add_sized(egui::vec2(ui.available_width(), CARD_HEIGHT), button)
            // Nested atoms hide their text from accesskit, so the button has to be named here.
            .accessible_name(title)
    })
    .inner
}
