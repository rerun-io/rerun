use egui::layers::ShapeIdx;
use egui::{
    Align, AtomLayout, Frame, IntoAtoms, Layout, Margin, Rect, Response, Sense, Shape, Ui,
    UiBuilder, WidgetInfo, WidgetType,
};

use crate::UiExt as _;

/// Thickness of the accent line under the selected tab.
const UNDERLINE_HEIGHT: i8 = 2;

/// Padding on either side of a tab's content.
const TAB_PADDING_X: i8 = 16;

/// Padding above a tab's content.
const TAB_PADDING_TOP: i8 = 8;

/// Padding between a tab's content and the underline.
const TAB_PADDING_BOTTOM: i8 = 4;

/// Gap between neighboring tabs.
const TAB_SPACING: f32 = 8.0;

/// A row of tabs, with the selected one underlined.
///
/// The underline slides over when another tab is selected.
///
/// The tabs sit on a separator that spans the full width of the enclosing panel, see
/// [`crate::UiExt::full_span`].
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
        let mut child_ui = ui.new_child(UiBuilder::new().layout(Layout::left_to_right(Align::Min)));
        child_ui.spacing_mut().item_spacing.x = TAB_SPACING;

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
    fn underline_ui(&self, selected_rect: Rect) {
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

        ui.painter().hline(
            left..=right,
            selected_rect.bottom() - (UNDERLINE_HEIGHT as f32) * 0.5,
            egui::Stroke::new(UNDERLINE_HEIGHT as f32, ui.tokens().selection_bg_fill),
        );
    }
}

impl Drop for TabBar<'_> {
    fn drop(&mut self) {
        let rect = self.child_ui.min_rect();
        let separator_y = rect.bottom() - f32::from(UNDERLINE_HEIGHT) * 0.5;

        self.child_ui.painter().set(
            self.separator,
            Shape::hline(
                self.child_ui.full_span(),
                separator_y,
                self.child_ui.visuals().widgets.noninteractive.bg_stroke,
            ),
        );

        if let Some(selected_rect) = self.selected_rect {
            self.underline_ui(selected_rect);
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

    let mut layout = AtomLayout::new(atoms)
        .frame(Frame::new().inner_margin(Margin {
            left: TAB_PADDING_X,
            right: TAB_PADDING_X,
            top: TAB_PADDING_TOP,
            bottom: TAB_PADDING_BOTTOM + UNDERLINE_HEIGHT,
        }))
        .sense(Sense::click())
        .allocate(ui);

    let text_color = if selected || layout.response.has_focus() {
        ui.visuals().strong_text_color()
    } else if layout.response.hovered() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
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
