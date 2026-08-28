use egui::layers::ShapeIdx;
use egui::{
    Align, AtomLayout, Frame, IntoAtoms, Layout, Margin, Rangef, Rect, Response, Sense, Shape, Ui,
    UiBuilder, WidgetInfo, WidgetType,
};

use crate::UiExt as _;

/// Thickness of the accent line under the selected tab.
const UNDERLINE_HEIGHT: i8 = 2;

/// Padding above a tab's content.
const TAB_PADDING_TOP: i8 = 8;

/// Padding between a tab's content and the underline.
const TAB_PADDING_BOTTOM: i8 = 4;

/// Gap between neighboring tabs.
///
/// A tab is only as wide as its label, so this is the whole gap between two labels.
const TAB_SPACING: f32 = 24.0;

/// How far the first tab sits from the left edge of the enclosing panel.
///
/// Matches the inset the rest of a dataset page uses, so the labels line up with the heading and
/// the cards below them.
const BAR_INSET_LEFT: f32 = 16.0;

/// How far short of the right edge of the enclosing panel the separator stops.
///
/// Same as [`BAR_INSET_LEFT`], so the line is inset as much as the cards below it. Only the
/// separator needs this. The tabs are packed against the left.
const BAR_INSET_RIGHT: f32 = BAR_INSET_LEFT;

/// Font size of a tab's label.
const TAB_FONT_SIZE: f32 = 12.0;

/// Height of the row between a [`TabBar`] and the content under it. The row summarizes that
/// content on the left and holds its actions on the right.
///
/// Every tab of a page uses this and [`TAB_TOOLBAR_MARGIN_Y`], so switching tabs doesn't move the
/// content up or down.
pub const TAB_TOOLBAR_HEIGHT: f32 = 26.0;

/// Space above and below a [`TAB_TOOLBAR_HEIGHT`] row.
pub const TAB_TOOLBAR_MARGIN_Y: f32 = 12.0;

/// A row of tabs, with the selected one underlined.
///
/// The underline slides over when another tab is selected.
///
/// The tabs sit on a separator that starts at the first tab and stops short of the right edge of
/// the enclosing panel.
///
/// ```
/// # egui::__run_test_ui(|ui| {
/// # #[derive(PartialEq)] enum Tab { Segments, Assets }
/// # let mut tab = Tab::Segments;
/// re_ui::TabBar::new(ui)
///     .selectable_value(&mut tab, Tab::Segments, "Segments")
///     .selectable_value(&mut tab, Tab::Assets, "Assets");
/// # });
/// ```
pub struct TabBar<'a> {
    ui: &'a mut Ui,
    child_ui: Ui,
    separator: ShapeIdx,

    /// Rect of the selected tab, once one has been added.
    selected_rect: Option<Rect>,
}

impl<'a> TabBar<'a> {
    pub fn new(ui: &'a mut Ui) -> Self {
        // Inset via the child's rect rather than `add_space`, so the tab spacing isn't added on
        // top of it before the first tab.
        let mut max_rect = ui.available_rect_before_wrap();
        max_rect.min.x += BAR_INSET_LEFT;

        let mut child_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(max_rect)
                .layout(Layout::left_to_right(Align::Min)),
        );
        child_ui.spacing_mut().item_spacing.x = TAB_SPACING;

        // Set here rather than per tab, so the bar keeps its own size if the body text size changes.
        child_ui.style_mut().override_font_id = Some(egui::FontId::proportional(TAB_FONT_SIZE));

        // Reserved up front so the separator ends up below the selected tab's underline.
        let separator = child_ui.painter().add(Shape::Noop);

        Self {
            ui,
            child_ui,
            separator,
            selected_rect: None,
        }
    }

    pub fn selectable_value<Value: PartialEq>(
        mut self,
        current_value: &mut Value,
        selected_value: Value,
        atoms: impl IntoAtoms<'a>,
    ) -> Self {
        let selected = *current_value == selected_value;
        let response = tab_ui(&mut self.child_ui, atoms, selected);

        if selected {
            self.selected_rect = Some(response.rect);
        }
        if response.clicked() {
            *current_value = selected_value;
        }

        self
    }

    /// Paints the accent line under the selected tab, sliding it over from wherever it was before.
    fn underline_ui(&self, selected_rect: Rect, baseline: f32) {
        let ui = &self.child_ui;
        let bar_left = ui.min_rect().left();

        // Animated relative to the left edge of the bar, so that moving the bar as a whole, for
        // instance when a panel is resized, doesn't drag the underline along.
        let (left, right) = if ui.is_sizing_pass() {
            (selected_rect.left(), selected_rect.right())
        } else {
            let id = ui.id().with("tab_underline");
            let animation_time = ui.style().animation_time * 0.5;
            let animate = |salt: &str, x: f32| {
                bar_left
                    + ui.ctx()
                        .animate_value_with_time(id.with(salt), x - bar_left, animation_time)
            };
            (
                animate("left", selected_rect.left()),
                animate("right", selected_rect.right()),
            )
        };

        // Same color as the selected label, so the line and the label match.
        underline(ui, left..=right, baseline, ui.tokens().tab.text_selected);
    }
}

/// Paints a tab underline sitting on `baseline`, so it is flush with the separator below it.
///
/// A stroke is centered on the line it is given, so both lines are offset up by half their own
/// thickness.
fn underline(ui: &Ui, x: std::ops::RangeInclusive<f32>, baseline: f32, color: egui::Color32) {
    let thickness = UNDERLINE_HEIGHT as f32;
    ui.painter().hline(
        x,
        baseline - thickness * 0.5,
        egui::Stroke::new(thickness, color),
    );
}

impl Drop for TabBar<'_> {
    fn drop(&mut self) {
        let rect = self.child_ui.min_rect();

        // The bottom of the bar is where both the separator and the underlines end, so the
        // thicker underline grows upwards from the separator instead of sitting across it.
        let baseline = rect.bottom();

        // Inset at both ends rather than spanning the panel. It starts where the first tab does
        // and stops as short of the right edge as the content below it.
        let span = Rangef::new(
            rect.left(),
            self.child_ui.max_rect().right() - BAR_INSET_RIGHT,
        );

        let divider = self.child_ui.visuals().widgets.noninteractive.bg_stroke;
        self.child_ui.painter().set(
            self.separator,
            Shape::hline(span, baseline - divider.width * 0.5, divider),
        );

        if let Some(selected_rect) = self.selected_rect {
            self.underline_ui(selected_rect, baseline);
        }

        self.ui.allocate_rect(rect, Sense::hover());
    }
}

/// Adds a single tab of a [`TabBar`].
///
/// The underline under the selected tab is painted by the bar itself.
fn tab_ui<'a>(ui: &mut Ui, atoms: impl IntoAtoms<'a>, selected: bool) -> Response {
    let atoms = atoms.into_atoms();
    let label = atoms.text().map(|text| text.into_owned());

    // A tab is as wide as its label: no horizontal padding, so the underline matches the word and
    // `TAB_SPACING` is the whole gap between two of them.
    let mut layout = AtomLayout::new(atoms)
        .frame(Frame::new().inner_margin(Margin {
            left: 0,
            right: 0,
            top: TAB_PADDING_TOP,
            bottom: TAB_PADDING_BOTTOM + UNDERLINE_HEIGHT,
        }))
        .sense(Sense::click())
        .allocate(ui);

    let tokens = ui.tokens();
    let hovered = layout.response.hovered();

    let text_color = if selected || layout.response.has_focus() {
        tokens.tab.text_selected
    } else if hovered {
        tokens.tab.text_hovered
    } else {
        tokens.tab.text
    };
    layout.fallback_text_color = text_color;
    layout.map_images(|image| image.tint(text_color));

    let response = layout.paint(ui).response;

    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label.as_deref().unwrap_or_default(),
        )
    });

    response
}

#[cfg(test)]
mod tests {
    use egui::Theme;
    use egui_kittest::kittest::Queryable as _;
    use egui_kittest::{Harness, SnapshotResults};

    use super::TabBar;

    #[derive(PartialEq)]
    enum Tab {
        Segments,
        Assets,
        Schema,
    }

    #[test]
    fn test_tab_bar() {
        let mut results = SnapshotResults::new();

        for theme in [Theme::Dark, Theme::Light] {
            let mut harness = Harness::builder()
                .with_theme(theme)
                .with_size(egui::vec2(400.0, 60.0))
                .build_ui(|ui| {
                    crate::apply_style_and_install_loaders(ui.ctx());

                    let mut tab = Tab::Assets;
                    TabBar::new(ui)
                        .selectable_value(&mut tab, Tab::Segments, "Segments")
                        .selectable_value(&mut tab, Tab::Assets, "Assets")
                        .selectable_value(&mut tab, Tab::Schema, "Schema");
                });

            harness.run();
            harness.snapshot(format!(
                "tab_bar_{}",
                match theme {
                    Theme::Dark => "dark",
                    Theme::Light => "light",
                }
            ));
            results.extend_harness(&mut harness);
        }
    }

    /// Selecting another tab slides the underline over instead of moving it in one jump, so a few
    /// frames after the click it should sit between the two tabs.
    #[test]
    fn test_tab_bar_underline_animation() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 60.0))
            .with_step_dt(1.0 / 60.0)
            .build_ui_state(
                |ui, tab| {
                    crate::apply_style_and_install_loaders(ui.ctx());

                    TabBar::new(ui)
                        .selectable_value(tab, Tab::Segments, "Segments")
                        .selectable_value(tab, Tab::Assets, "Assets")
                        .selectable_value(tab, Tab::Schema, "Schema");
                },
                Tab::Segments,
            );

        harness.run();

        // Clicked via accesskit so the pointer stays away and no tab ends up hovered.
        harness.get_by_label("Schema").click_accesskit();
        for _ in 0..3 {
            harness.step();
        }

        harness.snapshot("tab_bar_underline_animation");
    }
}
