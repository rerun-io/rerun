use egui::text::LayoutJob;

pub trait LayoutJobExt {
    /// Returns a copy of this job with the text and sections cleared, keeping the other
    /// layout settings (wrap, justify, `break_on_newline`, …).
    // TODO(emilk): add `LayoutJob::clear` to egui and use that instead.
    fn cleared(&self) -> LayoutJob;
}

impl LayoutJobExt for LayoutJob {
    fn cleared(&self) -> LayoutJob {
        Self {
            text: Default::default(),
            sections: Default::default(),
            ..self.clone()
        }
    }
}
