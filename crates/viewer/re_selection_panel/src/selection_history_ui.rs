use egui::IntoAtoms as _;
use re_ui::{UiExt as _, menu::menu_style};
use re_viewer_context::{Item, ItemCollection, ItemCounter, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use re_data_ui::item_title::QualifiedItemTitle;
use re_ui::HasDesignTokens as _;

use crate::selection_history::SelectionHistory;

const BUTTON_SIZE: f32 = 16.0;
const ICON_SIZE: f32 = 12.0;

/// Renders ← (back) then → (forward) history buttons.
///
/// Both buttons are always rendered so the layout never shifts.
/// Returns the `ItemCollection` to navigate to if a button was clicked.
pub fn selection_history_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    history: &mut SelectionHistory,
) -> Option<ItemCollection> {
    let prev = history_button_ui(
        ctx,
        viewport,
        ui,
        history,
        &re_ui::icons::BACK_SMALL,
        Direction::Back,
    );
    let next = history_button_ui(
        ctx,
        viewport,
        ui,
        history,
        &re_ui::icons::FORWARD_SMALL,
        Direction::Forward,
    );
    prev.or(next)
}

#[derive(Clone, Copy)]
enum Direction {
    Back,
    Forward,
}

/// Renders a single 16×16 history navigation button with a 12×12 icon.
///
/// Colors come from the `history_button_*` theme tokens; the icon is dimmed
/// when there is no history entry to navigate to in `direction`.
fn history_button_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    history: &mut SelectionHistory,
    icon: &re_ui::Icon,
    direction: Direction,
) -> Option<ItemCollection> {
    let has_target = match direction {
        Direction::Back => history.previous().is_some(),
        Direction::Forward => history.next().is_some(),
    };

    let button_size = egui::vec2(BUTTON_SIZE, BUTTON_SIZE);
    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::Button));

    if ui.is_rect_visible(rect) {
        let tokens = ui.tokens();
        let is_hovered = has_target && response.hovered();
        let fill = if is_hovered {
            tokens.history_button_fill_hovered
        } else {
            tokens.history_button_fill
        };
        let icon_tint = if has_target {
            tokens.history_button_icon_active
        } else {
            tokens.history_button_icon_inactive
        };
        let corner_radius = 4.0;

        ui.painter().rect_filled(rect, corner_radius, fill);

        let icon_rect =
            egui::Rect::from_center_size(rect.center(), egui::vec2(ICON_SIZE, ICON_SIZE));
        icon.as_image().tint(icon_tint).paint_at(ui, icon_rect);
    }

    if has_target && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Hover text describing the target selection.
    if has_target {
        let (dir_label, target_sel) = match direction {
            Direction::Back => ("previous", history.previous().map(|(_, s)| s)),
            Direction::Forward => ("next", history.next().map(|(_, s)| s)),
        };
        if let Some(title) = target_sel.and_then(|sel| EntryTitle::new(ctx, viewport, sel)) {
            let desc = title.text();
            response
                .clone()
                .on_hover_text(format!("Go to {dir_label} selection:\n{desc}"));
        }
    } else {
        let tip = match direction {
            Direction::Back => "No previous selections",
            Direction::Forward => "No future selections",
        };
        response.clone().on_disabled_hover_text(tip);
    }

    // Right-click opens a popup listing the whole history in this direction.
    if has_target {
        let mut navigated = false;
        egui::Popup::menu(&response)
            .style(menu_style())
            .open_memory(if response.secondary_clicked() {
                Some(egui::SetOpenCommand::Bool(true))
            } else if response.clicked() {
                // Close again if the button itself was clicked, since that navigates.
                Some(egui::SetOpenCommand::Bool(false))
            } else {
                None
            })
            .show(|ui| {
                let cur = history.current;
                let indices: Vec<usize> = match direction {
                    Direction::Back => (0..cur).rev().collect(),
                    Direction::Forward => ((cur + 1)..history.stack.len()).collect(),
                };
                for i in indices {
                    navigated |= history_item_ui(ctx, viewport, ui, i, history);
                }
            });
        if navigated {
            return history.current().cloned();
        }
    }

    // Click navigation.
    if has_target && response.clicked() {
        return match direction {
            Direction::Back => history.select_previous(),
            Direction::Forward => history.select_next(),
        };
    }

    None
}

/// Renders one history entry as a menu button. Returns `true` if it was clicked.
fn history_item_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    index: usize,
    history: &mut SelectionHistory,
) -> bool {
    let Some(selection) = history.stack.get(index).cloned() else {
        return false;
    };
    let Some(title) = EntryTitle::new(ctx, viewport, &selection) else {
        return false;
    };

    if ui.add(egui::Button::new(title.atoms)).clicked() {
        history.current = index;
        ui.close();
        true
    } else {
        false
    }
}

/// How one history entry is presented: the icon and label parts of the leading item.
struct EntryTitle {
    atoms: egui::Atoms<'static>,
}

impl EntryTitle {
    /// Returns `None` for an empty selection, which is never recorded in the history.
    fn new(
        ctx: &ViewerContext<'_>,
        viewport: &ViewportBlueprint,
        selection: &ItemCollection,
    ) -> Option<Self> {
        let style = ctx.egui_ctx().global_style();
        let icon_tint = style.tokens().label_button_icon_color;
        let items: Vec<&Item> = selection.iter_items().collect();
        let first = *items.first()?;
        let title = QualifiedItemTitle::from_item(ctx, viewport, &style, first);

        let atoms = if items.len() == 1 {
            title.into_atoms(&style, icon_tint)
        } else {
            items
                .iter()
                .copied()
                .collect::<ItemCounter>()
                .to_string()
                .into_atoms()
        };

        Some(Self { atoms })
    }

    fn text(&self) -> String {
        self.atoms.text().unwrap_or_default().to_string()
    }
}
