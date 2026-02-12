use egui::{RichText, WidgetText};
use std::sync::Arc;

pub trait WidgetTextExt {
    /// Override the font size. For [`WidgetText::Galley`], this will do nothing.
    fn force_size(self, size: f32) -> WidgetText;
}

impl WidgetTextExt for WidgetText {
    fn force_size(self, size: f32) -> WidgetText {
        match self {
            Self::Text(text) => RichText::new(text).size(size).into(),
            Self::RichText(mut text) => {
                let text_mut = Arc::make_mut(&mut text);
                *text_mut = std::mem::replace(text_mut, RichText::new("")).size(size);
                text.into()
            }
            Self::LayoutJob(mut job) => {
                let job_mut = Arc::make_mut(&mut job);
                job_mut.sections.iter_mut().for_each(|s| {
                    s.format.font_id.size = size;
                });
                job.into()
            }
            Self::Galley(galley) => {
                // nothing we can do here
                galley.into()
            }
        }
    }
}
