use egui::RichText;
use re_data_store::{log_db::LogDb, ObjectProps};
use re_log_types::LogMsg;

use crate::{data_ui::*, ui::Blueprint, Preview, Selection, ViewerContext};

use super::SpaceView;

// --- Selection panel ---

/// The "Selection View" side-bar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SelectionPanel {
    /// Persistent undo/redo.
    history: SelectionHistory,
}

impl SelectionPanel {
    #[allow(clippy::unused_self)]
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        egui_ctx: &egui::Context,
        blueprint: &mut Blueprint,
    ) {
        let shortcut = crate::ui::kb_shortcuts::TOGGLE_SELECTION_PANEL;
        blueprint.selection_panel_expanded ^= egui_ctx.input_mut().consume_shortcut(&shortcut);

        let panel_frame = ctx.design_tokens.panel_frame(egui_ctx);

        let collapsed = egui::SidePanel::right("selection_view_collapsed")
            .resizable(false)
            .frame(panel_frame)
            .default_width(16.0);
        let expanded = egui::SidePanel::right("selection_view_expanded")
            .resizable(true)
            .frame(panel_frame);

        egui::SidePanel::show_animated_between(
            egui_ctx,
            blueprint.selection_panel_expanded,
            collapsed,
            expanded,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed, or animating:
                    if ui
                        .small_button("⏴")
                        .on_hover_text(format!(
                            "Expand Selection View ({})",
                            egui_ctx.format_shortcut(&shortcut)
                        ))
                        .clicked()
                    {
                        blueprint.selection_panel_expanded = true;
                    }
                } else {
                    // Expanded:
                    if ui
                        .small_button("⏵")
                        .on_hover_text(format!(
                            "Collapse Selection View ({})",
                            egui_ctx.format_shortcut(&shortcut)
                        ))
                        .clicked()
                    {
                        blueprint.selection_panel_expanded = false;
                    }

                    ui.separator();

                    if let Some(selection) = self.history.show(ui, blueprint) {
                        ctx.rec_cfg.selection = selection;
                    }

                    self.contents(ui, ctx, blueprint);
                }
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
    ) {
        crate::profile_function!();

        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.inner_ui(ctx, blueprint, ui);
            });
    }

    #[allow(clippy::unused_self)]
    fn inner_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        ui: &mut egui::Ui,
    ) {
        match &ctx.rec_cfg.selection.clone() {
            Selection::None => {
                ui.weak("(nothing)");
            }
            Selection::MsgId(msg_id) => {
                // ui.label(format!("Selected msg_id: {:?}", msg_id));
                ui.label("Selected a specific log message");

                let msg = if let Some(msg) = ctx.log_db.get_log_msg(msg_id) {
                    msg
                } else {
                    re_log::warn!("Unknown msg_id selected. Resetting selection");
                    ctx.rec_cfg.selection = Selection::None;
                    return;
                };

                match msg {
                    LogMsg::BeginRecordingMsg(msg) => {
                        show_begin_recording_msg(ui, msg);
                    }
                    LogMsg::TypeMsg(msg) => {
                        show_type_msg(ctx, ui, msg);
                    }
                    LogMsg::DataMsg(msg) => {
                        show_detailed_data_msg(ctx, ui, msg);
                        ui.separator();
                        view_object(ctx, ui, &msg.data_path.obj_path, Preview::Medium);
                    }
                    LogMsg::PathOpMsg(msg) => {
                        show_path_op_msg(ctx, ui, msg);
                    }
                }
            }
            Selection::ObjTypePath(obj_type_path) => {
                ui.label(format!("Selected object type path: {}", obj_type_path));
            }
            Selection::Instance(instance_id) => {
                ui.label(format!("Selected object: {}", instance_id));
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    ctx.type_path_button(ui, instance_id.obj_path.obj_type_path());
                });
                ui.horizontal(|ui| {
                    ui.label("Object type:");
                    ui.label(obj_type_name(
                        ctx.log_db,
                        instance_id.obj_path.obj_type_path(),
                    ));
                });
                ui.separator();
                view_instance(ctx, ui, instance_id, Preview::Medium);
            }
            Selection::DataPath(data_path) => {
                ui.label(format!("Selected data path: {}", data_path));
                ui.horizontal(|ui| {
                    ui.label("Object path:");
                    ctx.obj_path_button(ui, &data_path.obj_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    ctx.type_path_button(ui, data_path.obj_path.obj_type_path());
                });
                ui.horizontal(|ui| {
                    ui.label("Object type:");
                    ui.label(obj_type_name(
                        ctx.log_db,
                        data_path.obj_path.obj_type_path(),
                    ));
                });

                ui.separator();

                view_data(ctx, ui, data_path);
            }
            Selection::Space(space) => {
                let space = space.clone();
                ui.label(format!("Selected space: {}", space));
                // I really don't know what we should show here.
            }
            Selection::SpaceView(space_view_id) => {
                if let Some(space_view) = blueprint.viewport.get_space_view_mut(space_view_id) {
                    ui.label("SpaceView");
                    ui_space_view(ctx, ui, space_view);
                } else {
                    ctx.rec_cfg.selection = Selection::None;
                }
            }
            Selection::SpaceViewObjPath(space_view_id, obj_path) => {
                if let Some(space_view) = blueprint.viewport.get_space_view_mut(space_view_id) {
                    egui::Grid::new("space_view_id_obj_path")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Space View:");
                            ctx.space_view_button_to(ui, &space_view.name, *space_view_id);
                            ui.end_row();

                            ui.label("Object Path:");
                            ctx.obj_path_button(ui, obj_path);
                            ui.end_row();
                        });

                    let mut props = space_view.obj_tree_properties.projected.get(obj_path);
                    obj_props_ui(ui, &mut props);
                    space_view
                        .obj_tree_properties
                        .individual
                        .set(obj_path.clone(), props);
                } else {
                    ctx.rec_cfg.selection = Selection::None;
                }
            }
        }
    }

    /// Updates the current selection.
    /// Intended to be called once per frame with the current selection for that frame.
    ///
    /// This is a no-op if `selection` == `current_selection`.
    pub fn update_selection(&mut self, selection: &Selection) {
        self.history.update_selection(selection);
    }
}

fn obj_type_name(log_db: &LogDb, obj_type_path: &ObjTypePath) -> String {
    if let Some(typ) = log_db.obj_db.types.get(obj_type_path) {
        format!("{typ:?}")
    } else {
        "<UNKNOWN>".to_owned()
    }
}

fn ui_space_view(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, space_view: &mut SpaceView) {
    egui::Grid::new("space_view").striped(true).show(ui, |ui| {
        ui.label("Name:");
        ui.label(&space_view.name);
        ui.end_row();

        ui.label("Path:");
        ctx.obj_path_button(ui, &space_view.space_path);
        ui.end_row();
    });

    ui.separator();

    use super::space_view::ViewCategory;
    match space_view.category {
        ViewCategory::ThreeD => {
            ui.label("3D view.");
            super::view_3d::show_settings_ui(ctx, ui, &mut space_view.view_state.state_3d);
        }
        ViewCategory::Tensor => {
            if let Some(state_tensor) = &mut space_view.view_state.state_tensor {
                ui.label("Tensor view.");
                state_tensor.ui(ui);
            }
        }
        ViewCategory::TwoD | ViewCategory::Text | ViewCategory::Plot => {}
    }
}

// --- Selection history ---

/// A `Selection` and its index into the historical stack.
#[derive(Debug, Clone)]
struct HistoricalSelection {
    index: usize,
    selection: Selection,
}

impl From<(usize, Selection)> for HistoricalSelection {
    fn from((index, selection): (usize, Selection)) -> Self {
        Self { index, selection }
    }
}

// ---

/// A stack of `Selection`s, used to implement "undo/redo"-like semantics for selections.
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SelectionHistory {
    current: usize, // index into `self.stack`
    stack: Vec<Selection>,
    show_detailed: bool,
}

impl SelectionHistory {
    pub fn current(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current)
            .cloned()
            .map(|s| (self.current, s).into())
    }
    pub fn previous(&self) -> Option<HistoricalSelection> {
        (self.current > 0).then(|| (self.current - 1, self.stack[self.current - 1].clone()).into())
    }
    pub fn next(&self) -> Option<HistoricalSelection> {
        (self.current < self.stack.len().saturating_sub(1))
            .then(|| (self.current + 1, self.stack[self.current + 1].clone()).into())
    }

    pub fn update_selection(&mut self, selection: &Selection) {
        // Selecting nothing is irrelevant from a history standpoint.
        if matches!(selection, Selection::None) {
            return;
        }

        // Do not grow the history if the thing being selected is equal to the value that the
        // current history cursor points to.
        if let Some(current) = self.current() {
            if current.selection == *selection {
                return;
            }
        }

        // Make sure to clear the entire redo history past this point: we are engaging in a
        // diverging timeline!
        if !self.stack.is_empty() {
            self.stack.drain(self.current + 1..);
        }

        self.stack.push(selection.clone());
        self.current = self.stack.len() - 1;
    }

    pub fn clear(&mut self) {
        self.current = 0;
        self.stack.clear();
    }

    pub fn show(&mut self, ui: &mut egui::Ui, blueprint: &Blueprint) -> Option<Selection> {
        let sel1 = ui
            // so the strip doesn't try and occupy the entire vertical space
            .horizontal(|ui| self.show_control_bar(ui, blueprint))
            .inner;

        if !self.show_detailed {
            return sel1.map(|sel| sel.selection);
        }

        let sel2 = ui
            .vertical(|ui| self.show_detailed_view(ui, blueprint))
            .inner;

        sel1.or(sel2).map(|sel| sel.selection)
    }

    fn show_control_bar(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        use egui_extras::{Size, StripBuilder};

        const BIG_BUTTON_SIZE: f32 = 50.0;
        const TINY_BUTTON_SIZE: f32 = 15.0;
        const MIN_COMBOBOX_SIZE: f32 = 100.0;

        let font_id = egui::TextStyle::Body.resolve(ui.style());

        let mut res = None;
        StripBuilder::new(ui)
            .size(Size::exact(BIG_BUTTON_SIZE)) // prev
            .size(Size::exact(TINY_BUTTON_SIZE)) // clear
            .size(Size::remainder().at_least(MIN_COMBOBOX_SIZE)) // browser
            .size(Size::exact(TINY_BUTTON_SIZE)) // expand/collapse
            .size(Size::exact(BIG_BUTTON_SIZE)) // next
            .horizontal(|mut strip| {
                // prev
                let mut prev = None;
                strip.cell(|ui| {
                    prev = self.show_prev_button(ui, blueprint);
                });

                // clear
                strip.cell(|ui| {
                    if ui
                        .small_button("↺")
                        .on_hover_text("Clear history")
                        .clicked()
                    {
                        self.clear();
                    }
                });

                // browser
                let mut picked = None;
                strip.cell(|ui| {
                    let clipped_width = ui.available_width() * 0.8; // leave some space for the icon!
                    picked = egui::ComboBox::from_id_source("history_browser")
                        .width(ui.available_width())
                        .wrap(false)
                        // TODO(cmc): ideally I would want this to show full selection string
                        // on hover (needs egui patch).
                        .selected_text(self.current().map_or_else(String::new, |sel| {
                            selection_to_clipped_string(
                                ui,
                                blueprint,
                                &sel.selection,
                                &font_id,
                                clipped_width,
                            )
                        }))
                        .show_ui(ui, |ui| {
                            for (i, sel) in self.stack.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    show_selection_index(ui, i);
                                    show_selection_kind(ui, sel);
                                    let sel = selection_to_clipped_string(
                                        ui,
                                        blueprint,
                                        sel,
                                        &font_id,
                                        clipped_width,
                                    );
                                    ui.selectable_value(&mut self.current, i, sel);
                                });
                            }
                        })
                        .inner
                        .and_then(|_| self.current());
                });

                // collapse/expand
                strip.cell(|ui| {
                    let shortcut = &crate::ui::kb_shortcuts::TOGGLE_SELECTION_DETAILED;
                    if ui
                    .small_button(if self.show_detailed { "⏶" } else { "⏷" })
                    .on_hover_text(format!(
                        "{} detailed history view ({})",
                        if self.show_detailed { "Collapse" } else { "Expand" },
                        ui.ctx().format_shortcut(shortcut)
                    ))
                    .clicked()
                    // TODO(cmc): feels like using the shortcut should highlight the associated
                    // button or something (but then again it, it'd make more sense to do that
                    // at the egui level rather than specifically here).
                    || ui.ctx().input_mut().consume_shortcut(shortcut)
                    {
                        self.show_detailed = !self.show_detailed;
                    }
                });

                // next
                let mut next = None;
                strip.cell(|ui| {
                    next = self.show_next_button(ui, blueprint);
                });

                res = prev.or(picked).or(next);
            });

        res
    }

    fn show_detailed_view(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        let mut picked = None;

        fn show_row(
            ui: &mut egui::Ui,
            blueprint: &Blueprint,
            enabled: bool,
            label: &str,
            sel: Option<HistoricalSelection>,
        ) -> bool {
            ui.label(label);

            let Some(sel) = sel else {
                ui.end_row();
                return false;
            };

            let clicked = ui
                .add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        show_selection_index(ui, sel.index);
                        show_selection_kind(ui, &sel.selection);
                        // No clipping for the detailed view: this will resize the side
                        // panel as needed!
                        ui.selectable_label(false, selection_to_string(blueprint, &sel.selection))
                            .clicked()
                    })
                    .inner
                })
                .inner;

            ui.end_row();

            clicked
        }

        egui::Grid::new("selection_history")
            .num_columns(3)
            .show(ui, |ui| {
                if show_row(ui, blueprint, true, "Previous", self.previous()) {
                    self.current -= 1;
                    picked = self.current();
                }

                _ = show_row(ui, blueprint, false, "Current", self.current());

                if show_row(ui, blueprint, true, "Next", self.next()) {
                    self.current += 1;
                    picked = self.current();
                }
            });

        picked
    }

    fn show_prev_button(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        const PREV_BUTTON: &str = "⏴ Prev";
        if let Some(previous) = self.previous() {
            let shortcut = &crate::ui::kb_shortcuts::SELECTION_PREVIOUS;
            let button_clicked = ui
                .small_button(PREV_BUTTON)
                .on_hover_text(format!(
                    "Go to previous selection ({}):\n[{}] {}",
                    ui.ctx().format_shortcut(shortcut),
                    previous.index,
                    selection_to_string(blueprint, &previous.selection),
                ))
                .clicked();
            // TODO(cmc): feels like using the shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            let shortcut_used = ui.ctx().input_mut().consume_shortcut(shortcut);
            if (button_clicked || shortcut_used) && previous.index != self.current {
                self.current = previous.index;
                return self.current();
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new(PREV_BUTTON)))
                .response
                .on_hover_text("No past selections found");
        }

        None
    }

    fn show_next_button(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        const NEXT_BUTTON: &str = "Next ⏵";
        if let Some(next) = self.next() {
            let shortcut = &crate::ui::kb_shortcuts::SELECTION_NEXT;
            let button_clicked = ui
                .small_button(NEXT_BUTTON)
                .on_hover_text(format!(
                    "Go to next selection ({}):\n[{}] {}",
                    ui.ctx().format_shortcut(shortcut),
                    next.index,
                    selection_to_string(blueprint, &next.selection),
                ))
                .clicked();
            // TODO(cmc): feels like using the shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            let shortcut_used = ui.ctx().input_mut().consume_shortcut(shortcut);
            if (button_clicked || shortcut_used) && next.index != self.current {
                self.current = next.index;
                return self.current();
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new(NEXT_BUTTON)))
                .response
                .on_hover_text("No future selections found");
        }

        None
    }
}

fn show_selection_index(ui: &mut egui::Ui, index: usize) {
    ui.weak(RichText::new(format!("{index:3}")).monospace());
}

// Different kinds of selections can share the same path in practice! We need to
// differentiate those in the UI to avoid confusion.
fn show_selection_kind(ui: &mut egui::Ui, sel: &Selection) {
    ui.weak(
        RichText::new(match sel {
            Selection::None => "NONE",
            Selection::MsgId(_) => "MSG",
            Selection::ObjTypePath(_) => "TYPE",
            Selection::Instance(_) => "INST",
            Selection::DataPath(_) => "DATA",
            Selection::Space(_) => "SPACE",
            Selection::SpaceView(_) => "VIEW",
        })
        .monospace(),
    );
}

fn selection_to_string(blueprint: &Blueprint, sel: &Selection) -> String {
    if let Selection::SpaceView(id) = sel {
        if let Some(space_view) = blueprint.viewport.get_space_view(id) {
            return space_view.name.clone();
        }
    }

    sel.to_string()
}

fn selection_to_clipped_string(
    ui: &mut egui::Ui,
    blueprint: &Blueprint,
    sel: &Selection,
    font_id: &egui::FontId,
    width: f32,
) -> String {
    let mut width = width - ui.fonts().glyph_width(font_id, '…');
    let mut sel = selection_to_string(blueprint, sel)
        .chars()
        .rev()
        .take_while(|c| {
            width -= ui.fonts().glyph_width(font_id, *c);
            width > 0.0
        })
        .collect::<String>();
    if width <= 0.0 {
        sel += "…";
    }
    sel.chars().rev().collect()
}

fn obj_props_ui(ui: &mut egui::Ui, obj_props: &mut ObjectProps) {
    let ObjectProps { visible } = obj_props;
    ui.checkbox(visible, "Visible");
}
