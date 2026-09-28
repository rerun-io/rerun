use crate::{DesignTokens, Size, UiExt as _, icons};
use eframe::emath::Align;
use eframe::epaint::FontFamily;
use eframe::epaint::text::TextWrapMode;
use egui::{
    Atom, AtomExt as _, Atoms, Button, ContainerAtom, Direction, FontId, Frame, IdSalt, Layout,
    Margin, Pos2, Rect, Response, Sense, TextStyle, Ui, UiBuilder, Vec2, Widget, WidgetAtom,
    WidgetText,
};

/// The value shown alongside a [`ComboItem`]'s label.
enum ComboItemValue<'a> {
    /// Plain text. This is the only variant that supports [`ComboItem::value_below`] (stacked
    /// layout), since text atoms paint natively inside a nested [`egui::AtomLayout`].
    Text(WidgetText),

    /// An arbitrary widget, drawn inline on the right.
    Widget(egui::BoxedWidget<'a>),
}

/// A selectable button to be used within [`egui::ComboBox`]es or [`egui::Popup`]s.
pub struct ComboItem<'a> {
    label: WidgetText,
    selected: bool,
    value: Option<ComboItemValue<'a>>,
    value_below: bool,
    error: Option<String>,
}

impl<'a> ComboItem<'a> {
    /// Create a new [`ComboItem`].
    pub fn new(label: impl Into<WidgetText>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            value: None,
            value_below: false,
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

    /// Add a value. Will be shown on the right side at font size 10 (or below the label if
    /// [`Self::value_below`] is set).
    pub fn value(mut self, value: impl Into<WidgetText>) -> Self {
        let value = value
            .into()
            .size(DesignTokens::combo_item_small_font_size());
        self.value = Some(ComboItemValue::Text(value));
        self
    }

    /// Add a value as a widget. Will be shown on the right side at font size 10.
    ///
    /// Note that widget values are always inline; [`Self::value_below`] only applies to text
    /// values (set via [`Self::value`]).
    pub fn value_widget(mut self, value: impl Widget + 'a) -> Self {
        self.value = Some(ComboItemValue::Widget(value.boxed()));
        self
    }

    /// Show the (text) value stacked *below* the label instead of inline on the right.
    ///
    /// Useful when the value is long (e.g. a fully-qualified name) and would otherwise squeeze the
    /// label. Only affects text values set via [`Self::value`].
    pub fn value_below(mut self, value_below: bool) -> Self {
        self.value_below = value_below;
        self
    }
}

impl Widget for ComboItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Implementation based on
        // https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=787-7335&m=dev
        // https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=695-4747&m=dev
        let Self {
            mut label,
            selected,
            value,
            value_below,
            error,
        } = self;

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

        let error_id = IdSalt::new("error");
        let value_atom_id = IdSalt::new("value");
        let value_scope_id = ui.next_auto_id().with("value_scope");

        // Nested `ContainerAtom`s hide their text from accesskit, so the stacked layout has to
        // name its button explicitly.
        let mut stacked_accessible_name = None;

        let response = if value_below && let Some(ComboItemValue::Text(value)) = &value {
            let gap = 2.0;
            stacked_accessible_name = Some(label.text().to_owned());

            let first_line = ContainerAtom::new((
                check_icon,
                Atom::from(label).atom_align(egui::Align2::LEFT_CENTER),
                Atom::grow(),
            ))
            .gap(gap);

            let second_line = ContainerAtom::new((
                Atom::default().atom_size(Vec2::new(check_icon_size.x, 0.0)),
                Atom::from(value.clone()).atom_align(egui::Align2::LEFT_CENTER),
                Atom::grow(),
            ))
            .gap(gap);

            let stacked = ContainerAtom::new((first_line, second_line))
                .direction(Direction::TopDown)
                .gap(gap);
            Button::new(stacked).min_size(Vec2::splat(Size::Small.height() + 10.0))
        } else {
            let mut atoms = Atoms::new((check_icon, label));

            if error.is_some() {
                atoms.push_right(Atom::grow().atom_size(Vec2::new(16.0, 0.0)));
                atoms.push_right(Atom::custom(error_id, ui.tokens().small_icon_size));
            } else if value.is_some() {
                let value_scope_response = ui.read_response(value_scope_id);
                let size = value_scope_response
                    .map(|r| r.rect.size())
                    .unwrap_or_default();

                atoms.push_right(Atom::grow().atom_size(Vec2::new(16.0, 0.0)));
                atoms.push_right(Atom::custom(value_atom_id, size));
            }

            // Since the ComboItem has uneven padding due to the checkmark, we need to manually add 4px
            // spacing (2px space + 2px gap = 4px)
            atoms.push_right(Atom::default().atom_size(Vec2::new(2.0, 0.0)));

            Button::new(atoms).wrap_mode(TextWrapMode::Extend)
        }
        .atom_ui(ui);

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
        } else if let Some(rect) = response.rect(value_atom_id)
            && let Some(value) = value
        {
            let rect = Rect::from_min_max(
                Pos2::new(
                    rect.max.x - DesignTokens::combo_item_max_value_width(),
                    rect.min.y,
                ),
                rect.max,
            );
            let mut child_ui = ui.new_child(
                UiBuilder::new()
                    .scope_id(value_scope_id)
                    .max_rect(rect)
                    .layout(Layout::right_to_left(Align::Center)),
            );

            child_ui.style_mut().interaction.selectable_labels = false;
            // Override the text size to match the design
            for text_style in [TextStyle::Body, TextStyle::Monospace, TextStyle::Button] {
                if let Some(font) = child_ui.style_mut().text_styles.get_mut(&text_style) {
                    font.size = DesignTokens::combo_item_small_font_size();
                }
            }

            match value {
                ComboItemValue::Text(text) => {
                    child_ui.label(text);
                }
                ComboItemValue::Widget(widget) => {
                    child_ui.add(widget);
                }
            }
        }

        match stacked_accessible_name {
            Some(name) => response.response.accessible_name(name),
            None => response.response,
        }
    }
}

/// A header to group multiple [`ComboItem`]s.
///
/// It will ensure the correct gap above and below the header.
pub struct ComboItemHeader {
    label: WidgetText,
}

impl ComboItemHeader {
    /// Create a new [`ComboItemHeader`].
    pub fn new(label: impl Into<WidgetText>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl Widget for ComboItemHeader {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add(
            WidgetAtom::new(self.label)
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
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

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
                })
                .response
                .accessible_name("Example");
        });

        harness.get_by_value("ComboItem Example").click();

        harness.run();
        harness.fit_contents();

        let options = crate::testing::default_snapshot_options_for_ui();

        harness.snapshot_options("combo_item", &options);
    }

    /// [`ComboItem::value_below`]: the value is stacked below the label rather than inline, so
    /// long values don't squeeze the label. Used e.g. by the table "Column display format" menu.
    #[test]
    pub fn test_combo_item_value_below() {
        let mut harness = Harness::new_ui(|ui| {
            crate::apply_style_and_install_loaders(ui.ctx());

            ComboBox::new("combo_item_value_below_example", "")
                .selected_text("ComboItem Example")
                .popup_style(menu_style())
                .height(300.0)
                .show_ui(ui, |ui| {
                    ui.add(
                        ComboItem::new("Compact")
                            .value("Start time")
                            .value_below(true),
                    );
                    ui.add(
                        ComboItem::new("Component")
                            .value("RecordingInfo:start_time")
                            .value_below(true),
                    );
                    ui.add(
                        ComboItem::new("Full")
                            .value("property:RecordingInfo:start_time")
                            .value_below(true)
                            .selected(true),
                    );
                })
                .response
                .accessible_name("Example");
        });

        harness.get_by_value("ComboItem Example").click();

        harness.run();
        harness.fit_contents();

        let options = crate::testing::default_snapshot_options_for_ui();

        harness.snapshot_options("combo_item_value_below", &options);
    }
}
