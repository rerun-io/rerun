use crate::UiExt as _;
use crate::re_form::{ConstructFormStrip, FormStrip, Fractions, form_field_frame};
use egui::epaint::RectShape;
use egui::layers::ShapeIdx;
use egui::{
    AtomLayout, Atoms, Direction, Frame, IntoAtoms, Layout, Response, Shape, Stroke, StrokeKind,
    Ui, Widget,
};
use std::ops::{Deref, DerefMut};

pub struct SelectableStrip<'a> {
    strip: FormStrip<'a>,
    bg_idx: ShapeIdx,
}

impl<'a> SelectableStrip<'a> {
    pub fn single(ui: &'a mut Ui, widget: impl Widget) -> Response {
        let mut fields = Self::same(ui, 1);
        fields.add(widget)
    }

    pub fn selectable_value<Value: PartialEq>(
        mut self,
        current_value: &mut Value,
        selected_value: Value,
        atoms: impl IntoAtoms<'a>,
    ) -> Self {
        let toggle = SelectableToggle::new(atoms, *current_value == selected_value);
        let mut response = self.strip.add(toggle);
        if response.clicked() {
            *current_value = selected_value;
            response.mark_changed();
        }
        self
    }
}

impl<'a> ConstructFormStrip<'a> for SelectableStrip<'a> {
    fn new(ui: &'a mut Ui, fields: Fractions) -> Self {
        // FormStrip::new reads spacing during construction so we override it here
        let item_spacing = std::mem::take(&mut ui.spacing_mut().item_spacing);
        let strip = FormStrip::new(ui, fields)
            .with_item_layout(Layout::centered_and_justified(Direction::LeftToRight));
        strip.ui.spacing_mut().item_spacing = item_spacing;

        let bg_idx = strip.child_ui.painter().add(Shape::Noop);

        Self { strip, bg_idx }
    }
}

impl<'a> Deref for SelectableStrip<'a> {
    type Target = FormStrip<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.strip
    }
}

impl DerefMut for SelectableStrip<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.strip
    }
}

impl Drop for SelectableStrip<'_> {
    fn drop(&mut self) {
        let rect = self.strip.child_ui.min_rect();
        let frame = form_field_frame(self.strip.child_ui.tokens());
        self.strip.child_ui.painter().set(
            self.bg_idx,
            Shape::Rect(RectShape::new(
                rect,
                frame.corner_radius,
                frame.fill,
                frame.stroke,
                StrokeKind::Inside,
            )),
        );
    }
}

pub struct SelectableToggle<'a> {
    atoms: Atoms<'a>,
    selected: bool,
}

impl<'a> SelectableToggle<'a> {
    pub fn new(atoms: impl IntoAtoms<'a>, selected: bool) -> Self {
        Self {
            atoms: atoms.into_atoms(),
            selected,
        }
    }
}

impl Widget for SelectableToggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Allocate widget space and observe interactions.
        let mut atom_layout = AtomLayout::new(self.atoms)
            .frame(Frame::new().corner_radius(4.0))
            .sense(egui::Sense::click())
            .allocate(ui);

        // Set selected style.
        if self.selected {
            atom_layout.frame = atom_layout
                .frame
                .stroke(Stroke::new(1.0, ui.tokens().form_selectable_stroke_color))
                .fill(ui.tokens().form_selectable_bg_color);
        }

        if atom_layout.response.hovered() {
            let hover_fill = ui.style().visuals.widgets.hovered.bg_fill;
            atom_layout.frame = atom_layout.frame.fill(hover_fill);
        }

        atom_layout.paint(ui).response
    }
}
