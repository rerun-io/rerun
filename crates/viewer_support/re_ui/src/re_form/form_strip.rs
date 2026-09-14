use eframe::emath::{Align, Vec2};
use egui::{Layout, Response, Sense, Ui, UiBuilder, Widget};
use smallvec::{SmallVec, smallvec};

pub type Fractions = SmallVec<[f32; 3]>;

/// Minimal, horizontal version of [`egui_extras::StripBuilder`] with a more convenient api.
pub struct FormStrip<'a> {
    fields: Fractions,
    available_width_without_gaps: f32,
    current_index: usize,
    pub(crate) ui: &'a mut Ui,
    item_layout: Layout,
    pub(crate) child_ui: Ui,
    response: Option<Response>,
}

impl FormStrip<'_> {
    pub fn with_item_layout(mut self, layout: Layout) -> Self {
        self.item_layout = layout;
        self
    }
}

impl<'a> FormStrip<'a> {
    pub fn single(ui: &'a mut Ui, widget: impl Widget) -> Response {
        let mut fields = Self::same(ui, 1);
        fields.add(widget)
    }

    fn new(ui: &'a mut Ui, fields: Fractions) -> Self {
        let item_layout = *ui.layout();

        let gap_size = ui.spacing().item_spacing.x;
        let total_available = ui.available_width();
        let available = total_available - gap_size * (fields.len() as f32 - 1.0) / 2.0; // TODO(lucas): Why is gap sized halved??

        let child_ui = ui.new_child(
            UiBuilder::new().layout(Layout::left_to_right(Align::Min).with_cross_align(Align::Min)),
        );

        // let child_ui: Ui = ui.new_child(UiBuilder::new());
        Self {
            fields,
            current_index: 0,
            available_width_without_gaps: available,
            item_layout,
            ui,
            child_ui,
            response: None,
        }
    }

    pub fn and(&mut self, widget: impl Widget) -> &mut Self {
        self.add(widget);
        self
    }

    pub fn add(&mut self, widget: impl Widget) -> Response {
        let index = self.current_index;
        self.current_index += 1;
        let fraction = self.fields[index];

        let width = self.available_width_without_gaps * fraction;

        let response = add_sized(
            &mut self.child_ui,
            Vec2::new(width, 24.0),
            self.item_layout,
            widget,
        );
        if let Some(combined_child_response) = &mut self.response {
            *combined_child_response |= response.clone();
        } else {
            self.response = Some(response.clone());
        }
        response
    }

    /// Can be called to acquire the combined [`Response`] from the child ui
    /// and the added widgets. Should be called last.
    pub fn done(&mut self) -> Response {
        let mut response = self.child_ui.response();
        if let Some(combined_child_response) = self.response.take() {
            response |= combined_child_response;
        }
        response
    }
}

/// Trait to easily add the construction fns to a wrapper type
pub trait ConstructFormStrip<'a>: Sized {
    fn same(ui: &'a mut Ui, count: usize) -> Self {
        let each = 1.0 / count as f32;
        Self::new(ui, smallvec![each; count])
    }

    fn relative(ui: &'a mut Ui, relative: impl IntoIterator<Item = f32>) -> Self {
        let mut fields: Fractions = relative.into_iter().collect();
        let sum: f32 = fields.iter().sum();
        fields.iter_mut().for_each(|fract| {
            *fract /= sum;
        });
        Self::new(ui, fields)
    }

    fn new(ui: &'a mut Ui, fields: Fractions) -> Self;
}

impl<'a> ConstructFormStrip<'a> for FormStrip<'a> {
    fn new(ui: &'a mut Ui, fields: Fractions) -> Self {
        FormStrip::new(ui, fields)
    }
}

impl Drop for FormStrip<'_> {
    fn drop(&mut self) {
        self.ui
            .allocate_rect(self.child_ui.min_rect(), Sense::hover());
    }
}

/// Copy of `ui.add_sized` that aligns items to the left
fn add_sized(
    ui: &mut Ui,
    max_size: impl Into<Vec2>,
    layout: Layout,
    widget: impl Widget,
) -> Response {
    ui.allocate_ui_with_layout(max_size.into(), layout, |ui| ui.add(widget))
        .inner
}
