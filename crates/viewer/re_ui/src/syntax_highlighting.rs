use re_entity_db::InstancePath;
use re_log_types::{
    ComponentPath, EntityPath, EntityPathPart, Instance,
    external::re_types_core::{
        ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType,
    },
};

use crate::DesignTokens;
use egui::{Color32, FontId, FontSelection, Style, TextFormat, text::LayoutJob};

// ----------------------------------------------------------------------------
pub trait SyntaxHighlighting {
    fn syntax_highlighted(&self, style: &Style) -> LayoutJob {
        let mut builder = SyntaxHighlightedBuilder::new(style);
        self.syntax_highlight_into(&mut builder);
        builder.job
    }

    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder);
}

// ----------------------------------------------------------------------------

/// Easily build syntax-highlighted text.
pub struct SyntaxHighlightedBuilder {
    pub tokens: &'static DesignTokens,
    pub job: LayoutJob,

    // In order to avoid having a lifetime on &Style, we just grab what we need:
    body: FontId,
    monospace: FontId,
    override_text_color: Option<Color32>,
}

/// Easily build syntax-highlighted [`LayoutJob`]s.
impl SyntaxHighlightedBuilder {
    pub const QUOTE_CHAR: char = '"';

    pub fn new(style: &Style) -> Self {
        Self {
            body: FontSelection::Default.resolve(&style),
            monospace: egui::TextStyle::Monospace.resolve(style),
            override_text_color: style.visuals.override_text_color,
            tokens: crate::design_tokens_of_visuals(&style.visuals),
            job: LayoutJob::default(),
        }
    }

    pub fn from(style: &Style, job: impl Into<LayoutJob>) -> Self {
        let mut builder = Self::new(style);
        builder.job = job.into();
        builder
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
        let format = self.monospace_with_color(self.tokens.code_string);
        let quote = Self::QUOTE_CHAR.to_string();
        self.job.append(&quote, 0.0, format.clone());
        self.job.append(portion, 0.0, format.clone());
        self.job.append(&quote, 0.0, format);
        self
    }

    /// A string identifier.
    ///
    /// E.g. a variable name, field name, etc. Won't be quoted.
    pub fn append_identifier(&mut self, portion: &str) -> &mut Self {
        let format = self.monospace_with_color(self.tokens.text_default);
        self.append_with_format(portion, format);
        self
    }

    /// An index number, e.g. an array index.
    pub fn append_index(&mut self, portion: &str) -> &mut Self {
        let format = self.monospace_with_color(self.tokens.code_index);
        self.append_with_format(portion, format);
        self
    }

    /// Some primitive value, e.g. a number or bool.
    pub fn append_primitive(&mut self, portion: &str) -> &mut Self {
        let format = self.monospace_with_color(self.tokens.code_primitive);
        self.append_with_format(portion, format);
        self
    }

    /// Some syntax, e.g. brackets, commas, colons, etc.
    pub fn append_syntax(&mut self, portion: &str) -> &mut Self {
        let format = self.monospace_with_color(self.tokens.text_strong);
        self.append_with_format(portion, format);
        self
    }

    pub fn append_body(&mut self, portion: &str) -> &mut Self {
        let format = self.body();
        self.append_with_format(portion, format);
        self
    }

    pub fn append_body_italics(&mut self, portion: &str) -> &mut Self {
        let mut format = self.body();
        format.italics = true;
        self.append_with_format(portion, format);
        self
    }

    #[inline]
    pub fn append_with_format(&mut self, text: &str, format: TextFormat) -> &mut Self {
        self.job.append(text, 0.0, format);
        self
    }

    #[inline]
    pub fn with_string_value(mut self, portion: &str) -> Self {
        self.append_string_value(portion);
        self
    }

    #[inline]
    pub fn with_syntax(mut self, portion: &str) -> Self {
        self.append_syntax(portion);
        self
    }

    #[inline]
    pub fn with_body(mut self, portion: &str) -> Self {
        self.append_body(portion);
        self
    }

    #[inline]
    pub fn with_index(mut self, portion: &str) -> Self {
        self.append_index(portion);
        self
    }

    #[inline]
    pub fn with_identifier(mut self, portion: &str) -> Self {
        self.append_identifier(portion);
        self
    }

    #[inline]
    pub fn with_primitive(mut self, portion: &str) -> Self {
        self.append_primitive(portion);
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

    /// Monospace text format with a specific color (that may be overridden by the style).
    pub fn monospace_with_color(&self, color: Color32) -> TextFormat {
        TextFormat {
            font_id: self.monospace.clone(),
            color: self.override_text_color.unwrap_or(color),
            ..Default::default()
        }
    }

    pub fn body(&self) -> TextFormat {
        TextFormat {
            font_id: self.body.clone(),
            color: self.override_text_color.unwrap_or(Color32::PLACEHOLDER),
            ..Default::default()
        }
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

impl SyntaxHighlighting for EntityPathPart {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(&self.ui_string());
    }
}

impl SyntaxHighlighting for Instance {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        if self.is_all() {
            builder.append_primitive("all");
        } else {
            builder.append_index(&re_format::format_uint(self.get()));
        }
    }
}

impl SyntaxHighlighting for EntityPath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
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
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append(&self.entity_path);
        if self.instance.is_specific() {
            builder.append(&InstanceInBrackets(self.instance));
        }
    }
}

impl SyntaxHighlighting for ComponentType {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.short_name());
    }
}

impl SyntaxHighlighting for ArchetypeName {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.as_str());
    }
}

impl SyntaxHighlighting for ComponentIdentifier {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.as_ref());
    }
}

impl SyntaxHighlighting for ComponentDescriptor {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_identifier(self.display_name());
    }
}

impl SyntaxHighlighting for ComponentPath {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
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
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder
            .append_syntax("[")
            .append(&self.0)
            .append_syntax("]");
    }
}

macro_rules! impl_sh_primitive {
    ($t:ty, $to_string:path) => {
        impl SyntaxHighlighting for $t {
            fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
                builder.append_primitive(&$to_string(*self));
            }
        }
    };
    ($t:ty) => {
        impl SyntaxHighlighting for $t {
            fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
                builder.append_primitive(&self.to_string());
            }
        }
    };
}

impl_sh_primitive!(f32, re_format::format_f32);
impl_sh_primitive!(f64, re_format::format_f64);

impl_sh_primitive!(i8, re_format::format_int);
impl_sh_primitive!(i16, re_format::format_int);
impl_sh_primitive!(i32, re_format::format_int);
impl_sh_primitive!(i64, re_format::format_int);
impl_sh_primitive!(isize, re_format::format_int);
impl_sh_primitive!(u8, re_format::format_uint);
impl_sh_primitive!(u16, re_format::format_uint);
impl_sh_primitive!(u32, re_format::format_uint);
impl_sh_primitive!(u64, re_format::format_uint);
impl_sh_primitive!(usize, re_format::format_uint);

impl_sh_primitive!(bool);

impl<T: SyntaxHighlighting> From<T> for SyntaxHighlightedBuilder {
    fn from(portion: T) -> Self {
        let mut builder = Self::new(&egui::Style::default());
        portion.syntax_highlight_into(&mut builder);
        builder
    }
}
