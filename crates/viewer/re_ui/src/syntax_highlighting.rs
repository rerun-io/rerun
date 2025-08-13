use std::sync::Arc;

use re_entity_db::InstancePath;
use re_log_types::{
    ComponentPath, EntityPath, EntityPathPart, Instance,
    external::re_types_core::{
        ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType,
    },
};

use crate::DesignTokens;
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
pub struct SyntaxHighlightedBuilder<'a> {
    pub style: &'a Style,
    pub tokens: &'a DesignTokens,
    pub job: LayoutJob,
}

/// Easily build syntax-highlighted [`LayoutJob`]s.
impl<'a> SyntaxHighlightedBuilder<'a> {
    pub fn new(style: &'a Style, tokens: &'a DesignTokens) -> Self {
        Self {
            style,
            tokens,
            job: LayoutJob::default(),
        }
    }

    #[inline]
    pub fn append(mut self, portion: &dyn SyntaxHighlighting) -> Self {
        portion.syntax_highlight_into(&self.style, &mut self.job);
        self
    }

    /// Some string data.
    pub fn code_string_value(&mut self, portion: &str) {
        let mut format = monospace_text_format(self.style);
        format.color = self.tokens.code_string;
        self.job.append(portion, 0.0, format);
    }

    /// A string name (e.g. the key of a map).
    pub fn code_name(&mut self, portion: &str) {
        let mut format = monospace_text_format(self.style);
        format.color = self.tokens.text_default;
        self.job.append(portion, 0.0, format);
    }

    /// An index number, e.g. an array index.
    pub fn code_index(&mut self, portion: &str) {
        let mut format = monospace_text_format(self.style);
        format.color = self.tokens.code_index;
        self.job.append(portion, 0.0, format);
    }

    /// Some primitive value, e.g. a number or bool.
    pub fn code_primitive(&mut self, portion: &str) {
        let mut format = monospace_text_format(self.style);
        // TODO: Rename token name
        format.color = self.tokens.code_number;
        self.job.append(portion, 0.0, format);
    }

    /// Some syntax, e.g. brackets, commas, colons, etc.
    pub fn code_syntax(&mut self, portion: &str) {
        let mut format = monospace_text_format(self.style);
        format.color = self.tokens.text_strong;
        self.job.append(portion, 0.0, format);
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

impl From<SyntaxHighlightedBuilder<'_>> for LayoutJob {
    fn from(builder: SyntaxHighlightedBuilder) -> Self {
        builder.into_job()
    }
}

impl From<SyntaxHighlightedBuilder<'_>> for egui::WidgetText {
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
        color: style.visuals.strong_text_color(),
        ..text_format(style)
    }
}

fn monospace_text_format(style: &Style) -> TextFormat {
    TextFormat {
        font_id: egui::TextStyle::Monospace.resolve(style),
        color: style.visuals.text_color(),
        ..Default::default()
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

impl SyntaxHighlighting for ComponentType {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.short_name().syntax_highlight_into(style, job);
    }
}

impl SyntaxHighlighting for ArchetypeName {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.short_name().syntax_highlight_into(style, job);
    }
}

impl SyntaxHighlighting for ComponentIdentifier {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.as_str().syntax_highlight_into(style, job);
    }
}

impl SyntaxHighlighting for ComponentDescriptor {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        self.display_name().syntax_highlight_into(style, job);
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
