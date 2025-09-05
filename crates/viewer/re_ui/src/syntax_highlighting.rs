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
        let mut builder = SyntaxHighlightedBuilder::new(style);
        self.syntax_highlight_into(&mut builder);
        builder.job
    }

    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>);
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
    pub fn new(style: &'a Style) -> Self {
        Self {
            style,
            tokens: crate::design_tokens_of_visuals(&style.visuals),
            job: LayoutJob::default(),
        }
    }

    #[inline]
    pub fn with(mut self, portion: &dyn SyntaxHighlighting) -> Self {
        portion.syntax_highlight_into(&mut self);
        self
    }

    #[inline]
    pub fn append(&mut self, portion: &dyn SyntaxHighlighting) -> &mut Self {
        portion.syntax_highlight_into(self);
        self
    }

    /// Some string data. Will be quoted.
    pub fn append_string_value(&mut self, portion: &str) -> &mut Self {
        let format = monospace_with_color(self.style, self.tokens.code_string_color);
        self.job.append("\"", 0.0, format.clone());
        self.job.append(portion, 0.0, format.clone());
        self.job.append("\"", 0.0, format);
        self
    }

    /// A string identifier.
    ///
    /// E.g. a variable name, field name, etc. Won't be quoted.
    pub fn append_identifier(&mut self, portion: &str) -> &mut Self {
        let format = monospace_with_color(self.style, self.tokens.text_default);
        self.job.append(portion, 0.0, format);
        self
    }

    /// An index number, e.g. an array index.
    pub fn append_index(&mut self, portion: &str) -> &mut Self {
        let format = monospace_with_color(self.style, self.tokens.code_index_color);
        self.job.append(portion, 0.0, format);
        self
    }

    /// Some primitive value, e.g. a number or bool.
    pub fn append_primitive(&mut self, portion: &str) -> &mut Self {
        let format = monospace_with_color(self.style, self.tokens.code_primitive_color);
        self.job.append(portion, 0.0, format);
        self
    }

    /// Some syntax, e.g. brackets, commas, colons, etc.
    pub fn append_syntax(&mut self, portion: &str) -> &mut Self {
        let format = monospace_with_color(self.style, self.tokens.text_strong);
        self.job.append(portion, 0.0, format);
        self
    }

    /// A filter operator
    pub fn append_filter_operator(&mut self, portion: &str) -> &mut Self {
        let format = body_text_with_color(self.style, self.tokens.filter_operator_color);
        self.job.append(portion, 0.0, format);
        self
    }

    #[inline]
    pub fn append_with_format(
        &mut self,
        text: &str,
        format: impl Fn(&'a Style) -> TextFormat,
    ) -> &mut Self {
        self.job.append(text, 0.0, format(self.style));
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

impl From<SyntaxHighlightedBuilder<'_>> for LayoutJob {
    fn from(builder: SyntaxHighlightedBuilder<'_>) -> Self {
        builder.into_job()
    }
}

impl From<SyntaxHighlightedBuilder<'_>> for egui::WidgetText {
    fn from(builder: SyntaxHighlightedBuilder<'_>) -> Self {
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

/// Body text with a specific color (that may be overridden by the style).
fn body_text_with_color(style: &Style, color: Color32) -> TextFormat {
    TextFormat {
        font_id: egui::TextStyle::Body.resolve(style),
        color: style.visuals.override_text_color.unwrap_or(color),
        ..Default::default()
    }
}

/// Monospace text format with a specific color (that may be overridden by the style).
fn monospace_with_color(style: &Style, color: Color32) -> TextFormat {
    TextFormat {
        font_id: egui::TextStyle::Monospace.resolve(style),
        color: style.visuals.override_text_color.unwrap_or(color),
        ..Default::default()
    }
}

impl SyntaxHighlighting for &'_ str {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_with_format(self, text_format);
    }
}

impl SyntaxHighlighting for String {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_with_format(self, text_format);
    }
}

impl SyntaxHighlighting for EntityPathPart {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_identifier(&self.ui_string());
    }
}

impl SyntaxHighlighting for Instance {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        if self.is_all() {
            builder.append_primitive("all");
        } else {
            builder.append_index(&re_format::format_uint(self.get()));
        }
    }
}

impl SyntaxHighlighting for EntityPath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_syntax("/");

        for (i, part) in self.iter().enumerate() {
            if i != 0 {
                builder.append_syntax("/");
            }
            builder.append(part);
        }
    }
}

impl SyntaxHighlighting for InstancePath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append(&self.entity_path);
        if self.instance.is_specific() {
            builder.append(&InstanceInBrackets(self.instance));
        }
    }
}

impl SyntaxHighlighting for ComponentType {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_identifier(self.short_name());
    }
}

impl SyntaxHighlighting for ArchetypeName {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_identifier(self.as_str());
    }
}

impl SyntaxHighlighting for ComponentIdentifier {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_identifier(self.as_ref());
    }
}

impl SyntaxHighlighting for ComponentDescriptor {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder.append_identifier(self.display_name());
    }
}

impl SyntaxHighlighting for ComponentPath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        let Self {
            entity_path,
            component_descriptor,
        } = self;
        builder
            .append(entity_path)
            .append_syntax(":")
            .append(component_descriptor);
    }
}

/// Formats an instance number enclosed in square brackets: `[123]`
pub struct InstanceInBrackets(pub Instance);

impl SyntaxHighlighting for InstanceInBrackets {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        builder
            .append_syntax("[")
            .append(&self.0)
            .append_syntax("]");
    }
}
