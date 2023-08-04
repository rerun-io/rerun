use egui::RichText;
use re_ui::UICommand;
use re_viewer_context::{Item, ItemCollection, SelectionHistory};
use re_viewport::ViewportBlueprint;

// ---

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SelectionHistoryUi {}

impl SelectionHistoryUi {
    pub(crate) fn selection_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        blueprint: &ViewportBlueprint<'_>,
        history: &mut SelectionHistory,
    ) -> Option<ItemCollection> {
        ui.horizontal_centered(|ui| {
            ui.strong("Selection").on_hover_text("The Selection View contains information and options about the currently selected object(s)");

            // TODO(emilk): an egui helper for right-to-left
            ui.allocate_ui_with_layout(
                ui.available_size_before_wrap(),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let next = self.next_button_ui(re_ui, ui, blueprint, history);
                    let prev = self.prev_button_ui(re_ui, ui, blueprint, history);
                    prev.or(next)
                }).inner
        }).inner
    }

    fn prev_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        blueprint: &ViewportBlueprint<'_>,
        history: &mut SelectionHistory,
    ) -> Option<ItemCollection> {
        // undo selection
        if let Some(previous) = history.previous() {
            let response = re_ui
                .small_icon_button(ui, &re_ui::icons::ARROW_LEFT)
                .on_hover_text(format!(
                    "Go to previous selection{}:\n\
                {}\n\
                \n\
                Right-click for more.",
                    UICommand::SelectionPrevious.format_shortcut_tooltip_suffix(ui.ctx()),
                    item_collection_to_string(blueprint, &previous.selection),
                ));

            let response = response.context_menu(|ui| {
                // undo: newest on top, oldest on bottom
                for i in (0..history.current).rev() {
                    self.history_item_ui(blueprint, ui, i, history);
                }
            });

            // TODO(cmc): using the keyboard shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            if response.clicked() {
                return history.select_previous();
            }
        } else {
            ui.add_enabled_ui(false, |ui| {
                re_ui
                    .small_icon_button(ui, &re_ui::icons::ARROW_LEFT)
                    .on_disabled_hover_text("No past selections found");
            });
        }

        None
    }

    fn next_button_ui(
        &mut self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        blueprint: &ViewportBlueprint<'_>,
        history: &mut SelectionHistory,
    ) -> Option<ItemCollection> {
        // redo selection
        if let Some(next) = history.next() {
            let response = re_ui
                .small_icon_button(ui, &re_ui::icons::ARROW_RIGHT)
                .on_hover_text(format!(
                    "Go to next selection{}:\n\
                {}\n\
                \n\
                Right-click for more.",
                    UICommand::SelectionNext.format_shortcut_tooltip_suffix(ui.ctx()),
                    item_collection_to_string(blueprint, &next.selection),
                ));

            let response = response.context_menu(|ui| {
                // redo: oldest on top, most recent on bottom
                for i in (history.current + 1)..history.stack.len() {
                    self.history_item_ui(blueprint, ui, i, history);
                }
            });

            // TODO(cmc): using the keyboard shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            if response.clicked() {
                return history.select_next();
            }
        } else {
            ui.add_enabled_ui(false, |ui| {
                re_ui
                    .small_icon_button(ui, &re_ui::icons::ARROW_RIGHT)
                    .on_disabled_hover_text("No future selections found");
            });
        }

        None
    }

    #[allow(clippy::unused_self)]
    fn history_item_ui(
        &mut self,
        blueprint: &ViewportBlueprint<'_>,
        ui: &mut egui::Ui,
        index: usize,
        history: &mut SelectionHistory,
    ) {
        if let Some(sel) = history.stack.get(index) {
            ui.horizontal(|ui| {
                {
                    // borrow checker workaround
                    let sel = item_collection_to_string(blueprint, sel);
                    if ui
                        .selectable_value(&mut history.current, index, sel)
                        .clicked()
                    {
                        ui.close_menu();
                    }
                }
                if sel.len() == 1 {
                    item_kind_ui(ui, sel.iter().next().unwrap());
                }
            });
        }
    }
}

// Different kinds of selections can share the same path in practice! We need to
// differentiate those in the UI to avoid confusion.
fn item_kind_ui(ui: &mut egui::Ui, sel: &Item) {
    ui.weak(RichText::new(format!("({})", sel.kind())));
}

fn item_collection_to_string(blueprint: &ViewportBlueprint<'_>, items: &ItemCollection) -> String {
    assert!(!items.is_empty()); // history never contains empty selections.
    if items.len() == 1 {
        item_to_string(blueprint, items.iter().next().unwrap())
    } else if let Some(kind) = items.are_all_same_kind() {
        format!("{}x {}s", items.len(), kind)
    } else {
        "<multiple selections>".to_owned()
    }
}

fn item_to_string(blueprint: &ViewportBlueprint<'_>, item: &Item) -> String {
    match item {
        Item::SpaceView(sid) => {
            if let Some(space_view) = blueprint.space_view(sid) {
                space_view.display_name.clone()
            } else {
                "<removed Space View>".to_owned()
            }
        }
        Item::InstancePath(_, entity_path) => entity_path.to_string(),
        Item::DataBlueprintGroup(sid, handle) => {
            if let Some(space_view) = blueprint.space_view(sid) {
                if let Some(group) = space_view.data_blueprint.group(*handle) {
                    group.display_name.clone()
                } else {
                    format!("<removed Group in {}>", space_view.display_name)
                }
            } else {
                "<Group in removed Space View>".to_owned()
            }
        }
        Item::ComponentPath(path) => {
            format!("{} {}", path.entity_path, path.component_name.short_name(),)
        }
    }
}
