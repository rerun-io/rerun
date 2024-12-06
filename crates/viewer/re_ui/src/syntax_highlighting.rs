use std::sync::Arc;

use re_entity_db::InstancePath;
use re_log_types::{EntityPath, EntityPathPart, Instance};

use egui::{text::LayoutJob, Color32, Style, TextFormat};

// ----------------------------------------------------------------------------
pub trait SyntaxHighlighting {
    fn syntax_highlighted(&self, style: &Style) -> LayoutJob {
        let mut job = LayoutJob::default();
        self.syntax_highlight_into(style, &mut job);
        job
    }

    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob);
}

// ----------------------------------------------------------------------------

/// Easily build syntax-highlighted text.
pub struct SyntaxHighlightedBuilder {
    pub style: Arc<Style>,
    pub job: LayoutJob,
}

/// Easilut build
impl SyntaxHighlightedBuilder {
    pub fn new(style: Arc<Style>) -> Self {
        Self {
            style,
            job: LayoutJob::default(),
        }
    }

    #[inline]
    pub fn append(mut self, portion: &dyn SyntaxHighlighting) -> Self {
        portion.syntax_highlight_into(&self.style, &mut self.job);
        self
    }

    #[inline]
    pub fn into_job(self) -> LayoutJob {
        self.job
    }

    #[inline]
    pub fn into_widget_text(self) -> egui::WidgetText {
        self.into_job().into()
    }
}

impl From<SyntaxHighlightedBuilder> for LayoutJob {
    fn from(builder: SyntaxHighlightedBuilder) -> Self {
        builder.into_job()
    }
}

impl From<SyntaxHighlightedBuilder> for egui::WidgetText {
    fn from(builder: SyntaxHighlightedBuilder) -> Self {
        builder.into_widget_text()
    }
}

// ----------------------------------------------------------------------------

fn text_format(style: &Style) -> TextFormat {
    TextFormat {
        font_id: egui::TextStyle::Body.resolve(style),

        // This color be replaced with appropriate color based on widget,
        // and whether the widget is hovered, etc
        color: Color32::PLACEHOLDER,

        ..Default::default()
    }
}

fn faint_text_format(style: &Style) -> TextFormat {
    TextFormat {
        color: Color32::WHITE,

        ..text_format(style)
    }
}

impl SyntaxHighlighting for String {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append(self, 0.0, text_format(style));
    }
}

impl SyntaxHighlighting for EntityPathPart {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append(&self.ui_string(), 0.0, text_format(style));
    }
}

impl SyntaxHighlighting for Instance {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        if self.is_all() {
            job.append("all", 0.0, text_format(style));
        } else {
            job.append(&re_format::format_uint(self.get()), 0.0, text_format(style));
        }
    }
}

impl SyntaxHighlighting for EntityPath {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append("/", 0.0, faint_text_format(style));

        for (i, part) in self.iter().enumerate() {
            if i != 0 {
                job.append("/", 0.0, faint_text_format(style));
            }
            part.syntax_highlight_into(style, job);
        }
    }
}

impl SyntaxHighlighting for InstancePath {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.entity_path.syntax_highlight_into(style, job);
        if self.instance.is_specific() {
            InstanceInBrackets(self.instance).syntax_highlight_into(style, job);
        }
    }
}

/// Formats an instance number enclosed in square brackets: `[123]`
pub struct InstanceInBrackets(pub Instance);

impl SyntaxHighlighting for InstanceInBrackets {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append("[", 0.0, faint_text_format(style));
        self.0.syntax_highlight_into(style, job);
        job.append("]", 0.0, faint_text_format(style));
    }
}
