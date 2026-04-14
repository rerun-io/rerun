use egui::{
    Align2, Atom, AtomLayoutResponse, Color32, Frame, Id, NumExt as _, Sense, Shadow, Stroke, Ui,
    UiBuilder, Vec2,
};

use re_ui::UiExt as _;
use re_ui::egui_ext::Group;

/// Configuration for the legend container widget.
pub struct LegendConfig {
    /// Where should the legend be shown within the plot?
    pub position: Align2,

    /// The base ID used to derive a predictable frame ID for the legend.
    /// Use [`legend_frame_id`] with the same ID to check hover state from outside.
    pub id: Id,
}

impl Default for LegendConfig {
    fn default() -> Self {
        Self {
            position: Align2::RIGHT_TOP,
            id: Id::new("plot_legend"),
        }
    }
}

/// Returns the ID used for the legend's frame.
pub fn legend_frame_id(id: Id) -> Id {
    id.with("legend_frame")
}

/// A standalone plot legend container.
pub struct LegendWidget {
    config: LegendConfig,
}

impl LegendWidget {
    pub fn new(config: LegendConfig) -> Self {
        Self { config }
    }

    /// Render the legend container overlaid on the given UI.
    pub fn show(&self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
        let frame_id = legend_frame_id(self.config.id);

        Group::new("legend")
            .align2(self.config.position)
            .show(ui, |ui| {
                Frame::popup(ui.style())
                    .outer_margin(4)
                    .inner_margin(4)
                    .shadow(Shadow::NONE)
                    .show(ui, |ui| {
                        ui.scope_builder(UiBuilder::new().id(frame_id), |ui| {
                            ui.set_max_width(300.0);

                            let max_height = (ui.available_height() * 0.8).at_most(300.0);
                            egui::ScrollArea::vertical()
                                .max_height(max_height)
                                .show(ui, |ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::LEFT),
                                        add_contents,
                                    );
                                });
                        });
                    });
            });
    }

    /// High-level API: render legend entries with built-in click-toggle and Alt+click solo/restore.
    ///
    /// Accepts a flat iterator of [`LegendEntry`] values (one per series/item).
    /// Entries sharing the same label are grouped into a single legend row.
    /// The returned [`LegendOutput::hidden_ids`] covers all IDs that should be hidden.
    pub fn show_entries(
        &self,
        ui: &mut Ui,
        entries: impl IntoIterator<Item = LegendEntry>,
    ) -> LegendOutput {
        // Group flat entries by label, preserving insertion order.
        let mut entry_map: indexmap::IndexMap<String, LegendEntryWidget> =
            indexmap::IndexMap::new();
        for e in entries {
            entry_map
                .entry(e.label.clone())
                .and_modify(|w| w.ids.push(e.id))
                .or_insert_with(|| LegendEntryWidget {
                    label: e.label,
                    color: e.color,
                    visible: e.visible,
                    hovered: e.hovered,
                    ids: vec![e.id],
                });
        }
        let grouped: Vec<LegendEntryWidget> = entry_map.into_values().collect();

        if grouped.is_empty() {
            return LegendOutput {
                hovered_id: None,
                hidden_ids: egui::IdSet::default(),
            };
        }

        let mut hovered_id: Option<Id> = None;
        let mut toggled_labels: egui::ahash::HashSet<&str> = egui::ahash::HashSet::default();
        let mut focus_label: Option<&str> = None;

        self.show(ui, |ui| {
            for entry in &grouped {
                let response = entry.show(ui);

                if response.hovered() {
                    hovered_id = entry.ids.first().copied();
                }
                if response.clicked() {
                    if ui.input(|r| r.modifiers.alt) {
                        focus_label = Some(entry.label.as_str());
                    } else {
                        toggled_labels.insert(entry.label.as_str());
                    }
                }
            }
        });

        let hidden_ids = if let Some(focus) = focus_label {
            let already_solo = grouped
                .iter()
                .all(|e| e.visible == (e.label.as_str() == focus));
            if already_solo {
                egui::IdSet::default()
            } else {
                grouped
                    .iter()
                    .filter(|e| e.label.as_str() != focus)
                    .flat_map(|e| e.ids.iter().copied())
                    .collect()
            }
        } else {
            grouped
                .iter()
                .filter(|e| {
                    let was_clicked = toggled_labels.contains(e.label.as_str());
                    let now_visible = e.visible != was_clicked;
                    !now_visible
                })
                .flat_map(|e| e.ids.iter().copied())
                .collect()
        };

        LegendOutput {
            hovered_id,
            hidden_ids,
        }
    }
}

/// Result of [`LegendWidget::show_entries`].
pub struct LegendOutput {
    /// ID of the hovered legend entry, if any.
    pub hovered_id: Option<Id>,

    /// IDs (from the input entries) that should be hidden after processing clicks this frame.
    pub hidden_ids: egui::IdSet,
}

/// Flat input for a single series/item. Pass an iterator of these to
/// [`LegendWidget::show_entries`], which groups them by label internally.
pub struct LegendEntry {
    pub id: Id,
    pub label: String,
    pub color: Color32,
    pub visible: bool,
    pub hovered: bool,
}

/// A single legend row (one per unique label). Built internally by [`LegendWidget::show_entries`].
struct LegendEntryWidget {
    label: String,
    color: Color32,
    visible: bool,
    hovered: bool,
    ids: Vec<Id>,
}

impl LegendEntryWidget {
    fn show(&self, ui: &mut Ui) -> egui::Response {
        let tokens = ui.tokens();
        let text_color = if self.hovered {
            tokens.list_item_strong_text
        } else if self.visible {
            tokens.list_item_noninteractive_text
        } else {
            tokens.list_item_noninteractive_text.gamma_multiply(0.5)
        };

        let text = egui::RichText::new(&self.label).color(text_color);

        let atoms = egui::Atoms::new((LegendSwatch::atom(), text));

        let mut atom_layout = egui::AtomLayout::new(atoms)
            .gap(4.0)
            .frame(Frame::NONE.inner_margin(egui::Margin::symmetric(4, 0)))
            .sense(Sense::click())
            .allocate(ui);

        atom_layout.response = atom_layout
            .response
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        let atom_response = atom_layout.paint(ui);

        // Paint the color dot / outline.
        LegendSwatch {
            color: self.color,
            visible: self.visible,
        }
        .paint(ui, &atom_response);

        atom_response.response
    }
}

pub struct LegendSwatch {
    pub color: Color32,
    pub visible: bool,
}

impl LegendSwatch {
    fn id() -> Id {
        Id::new("legend_swatch")
    }

    const SWATCH_SIZE: f32 = 8.0;

    pub fn atom() -> Atom<'static> {
        egui::Atom::custom(Self::id(), Vec2::splat(Self::SWATCH_SIZE))
    }

    pub fn paint(self, ui: &Ui, response: &AtomLayoutResponse) {
        let display_color = if self.visible {
            self.color
        } else {
            self.color.gamma_multiply(0.5)
        };

        if let Some(rect) = response.rect(Self::id()) {
            if self.visible {
                ui.painter()
                    .circle_filled(rect.center(), 4.0, display_color);
            } else {
                // Neutral gray outline when hidden (inset by half stroke width to match filled size).
                let stroke_color = ui.tokens().text_subdued;
                ui.painter()
                    .circle_stroke(rect.center(), 3.5, Stroke::new(1.0, stroke_color));
            }
        }
    }
}
