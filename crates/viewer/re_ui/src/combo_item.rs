use crate::egui_ext::WidgetTextExt as _;
use crate::{UiExt as _, icons};
use eframe::epaint::FontFamily;
use egui::{
    Atom, AtomExt as _, AtomLayout, Atoms, Button, FontId, Frame, Id, Margin, Response, Sense, Ui,
    Vec2, Widget, WidgetText,
};

/// A selectable button to be used within [`egui::ComboBox`]es or [`egui::Popup`]s.
pub struct ComboItem {
    label: WidgetText,
    selected: bool,
    value: Option<WidgetText>,
    error: Option<String>,
}

impl ComboItem {
    /// Create a new [`ComboItem`].
    pub fn new(label: impl Into<WidgetText>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            value: None,
            error: None,
        }
    }

    /// Show an error icon instead of the value on the right side.
    ///
    /// If the text isn't `""`, a tooltip with the message will be shown on hover.
    pub fn error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }

    /// Mark the item as selected. A check icon will be shown to the left of it.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Add a value. Will be shown on the right side at font size 10.
    pub fn value(mut self, value: impl Into<WidgetText>) -> Self {
        self.value = Some(value.into());
        self
    }
}

impl Widget for ComboItem {
    fn ui(self, ui: &mut Ui) -> Response {
        // Implementation based on
        // https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=787-7335&m=dev
        // https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=695-4747&m=dev
        let Self {
            mut label,
            selected,
            value,
            error,
        } = self;

        let small_font_size = 10.0;
        ui.spacing_mut().icon_spacing = 2.0;
        ui.spacing_mut().button_padding.x = 0.0;

        if error.is_some() {
            label = label.color(ui.tokens().error_fg_color);
        }

        let check_icon_size = Vec2::splat(12.0);
        let check_icon = if selected {
            icons::CHECKED
                .as_image()
                .tint(ui.tokens().text_strong)
                .atom_size(check_icon_size)
        } else {
            Atom::default().atom_size(check_icon_size)
        };

        let mut atoms = Atoms::new((check_icon, label));

        let error_id = Id::new("error");

        if error.is_some() {
            atoms.push_right(Atom::grow().atom_size(Vec2::new(16.0, 0.0)));
            atoms.push_right(Atom::custom(error_id, ui.tokens().small_icon_size));
        } else if let Some(mut value) = value {
            // Ensure the value is shown at the right font size
            value = value.force_size(small_font_size);
            atoms.push_right(Atom::grow().atom_size(Vec2::new(16.0, 0.0)));
            atoms.push_right(value.atom_max_width(124.0));
        }

        // Since the ComboItem has uneven padding due to the checkmark, we need to manually add 4px
        // spacing (2px space + 2px gap = 4px)
        atoms.push_right(Atom::default().atom_size(Vec2::new(2.0, 0.0)));

        let response = Button::new(atoms).atom_ui(ui);

        // Paint the error icon and tooltip
        if let Some(rect) = response.rect(error_id) {
            icons::ERROR
                .as_image()
                .tint(ui.tokens().alert_error.icon)
                .paint_at(ui, rect);

            if let Some(error) = error
                && !error.is_empty()
            {
                ui.interact(
                    rect,
                    response.response.id.with("error_hover"),
                    Sense::hover(),
                )
                .on_hover_text(error);
            }
        }

        response.response
    }
}

/// A header to group multiple [`ComboItem`]s.
///
/// It will ensure the correct gap above and below the header.
pub struct ComboItemHeader(WidgetText);

impl ComboItemHeader {
    /// Create a new [`ComboItemHeader`].
    pub fn new(label: impl Into<WidgetText>) -> Self {
        Self(label.into())
    }
}

impl Widget for ComboItemHeader {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add(
            AtomLayout::new(self.0)
                .frame(Frame::new().inner_margin(Margin {
                    bottom: 0,
                    left: 14, // 12 for check icon + 2 gap
                    right: 4,
                    top: 4,
                }))
                .min_size(Vec2::new(0.0, 22.0))
                .fallback_font(FontId::new(10.0, FontFamily::Proportional)),
        )
    }
}

#[cfg(test)]
pub mod tests {
    use crate::menu::menu_style;
    use crate::syntax_highlighting::SyntaxHighlightedBuilder;
    use crate::{ComboItem, ComboItemHeader};
    use egui::ComboBox;
    use egui_kittest::kittest::Queryable as _;
    use egui_kittest::{Harness, SnapshotOptions};

    #[test]
    pub fn test_combo_item() {
        let mut harness = Harness::new_ui(|ui| {
            crate::apply_style_and_install_loaders(ui.ctx());

            ComboBox::new("combo_item_example", "")
                .selected_text("ComboItem Example")
                .popup_style(menu_style())
                .height(300.0)
                .show_ui(ui, |ui| {
                    ui.add(ComboItemHeader::new("Recommended:"));

                    ui.add(
                        ComboItem::new("vertex_normals")
                            .error(Some("Invalid selector".to_owned()))
                            .selected(true),
                    );

                    let mut code = SyntaxHighlightedBuilder::new();
                    code.append_syntax("[")
                        .append_primitive("0.000")
                        .append_syntax(",")
                        .append_primitive("0.000")
                        .append_syntax("]");

                    ui.add(ComboItemHeader::new("Other values:"));
                    ui.add(ComboItem::new("vertex_positions"));
                    ui.add(
                        ComboItem::new("Rerun default").value(code.into_widget_text(ui.style())),
                    );
                });
        });

        harness.get_by_value("ComboItem Example").click();

        harness.run();
        harness.fit_contents();

        harness.snapshot_options(
            "combo_item",
            &SnapshotOptions::new().failed_pixel_count_threshold(10),
        );
    }
}
