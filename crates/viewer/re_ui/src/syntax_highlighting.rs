use std::sync::Arc;

use re_entity_db::InstancePath;
use re_log_types::{
    ComponentPath, EntityPath, EntityPathPart, Instance,
    external::re_types_core::{
        ArchetypeFieldName, ArchetypeName, ComponentDescriptor, ComponentName,
    },
};

use egui::{Color32, Style, TextFormat, text::LayoutJob};

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
        color: if style.visuals.dark_mode {
            Color32::WHITE
        } else {
            Color32::BLACK
        },

        ..text_format(style)
    }
}

impl SyntaxHighlighting for &'_ str {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        job.append(self.to_owned(), 0.0, text_format(style));
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

impl SyntaxHighlighting for ComponentName {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.short_name().syntax_highlight_into(style, job);
    }
}

impl SyntaxHighlighting for ArchetypeName {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.short_name().syntax_highlight_into(style, job);
    }
}

impl SyntaxHighlighting for ArchetypeFieldName {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.as_str().syntax_highlight_into(style, job);
    }
}

impl SyntaxHighlighting for ComponentDescriptor {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        if let Some(archetype_name) = &self.archetype_name {
            archetype_name.syntax_highlight_into(style, job);
            job.append(":", 0.0, faint_text_format(style));
        }

        self.component_name
            .short_name()
            .syntax_highlight_into(style, job);

        if let Some(archetype_field_name) = &self.archetype_field_name {
            job.append("#", 0.0, faint_text_format(style));
            archetype_field_name.syntax_highlight_into(style, job);
        }
    }
}

impl SyntaxHighlighting for ComponentPath {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        let Self {
            entity_path,
            component_descriptor,
        } = self;
        entity_path.syntax_highlight_into(style, job);
        job.append(":", 0.0, faint_text_format(style));
        component_descriptor.syntax_highlight_into(style, job);
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
