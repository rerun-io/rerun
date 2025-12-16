use egui::text::LayoutJob;
use egui::{Color32, Style, TextFormat, TextStyle};
use re_entity_db::InstancePath;
use re_log_types::external::re_types_core::{
    ArchetypeName, ComponentDescriptor, ComponentIdentifier, ComponentType,
};
use re_log_types::{ComponentPath, EntityPath, EntityPathPart, Instance};

use crate::HasDesignTokens as _;

// ----------------------------------------------------------------------------
pub trait SyntaxHighlighting {
    fn syntax_highlighted(&self, style: &Style) -> LayoutJob {
        let mut builder = SyntaxHighlightedBuilder::new();
        self.syntax_highlight_into(&mut builder);
        builder.into_job(style)
    }

    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder);
}

// ----------------------------------------------------------------------------

/// Easily build syntax-highlighted text.
#[derive(Debug, Default)]
pub struct SyntaxHighlightedBuilder {
    text: String,
    parts: smallvec::SmallVec<[SyntaxHighlightedPart; 1]>,
}

/// Easily build syntax-highlighted [`LayoutJob`]s.
///
/// Try to use one of the `append_*` or `with_*` methods that semantically matches
/// what you are trying to highlight. Check the docs of the `append_*` methods for examples
/// of what they should be used with.
///
/// The `with_*` methods are builder-style, taking `self` and returning `Self`.
/// The `append_*` methods take `&mut self` and return `&mut Self`.
///
/// Use the `with_*` methods when building something inline.
impl SyntaxHighlightedBuilder {
    pub const QUOTE_CHAR: char = '"';

    pub fn new() -> Self {
        Self::default()
    }

    /// Construct [`Self`] from an existing [`LayoutJob`].
    ///
    /// Some information (the `leading_space`) will be lost.
    pub fn from(job: impl Into<LayoutJob>) -> Self {
        let job = job.into();
        Self {
            text: job.text,
            parts: job
                .sections
                .into_iter()
                .map(|s| SyntaxHighlightedPart {
                    style: SyntaxHighlightedStyle::Custom(Box::new(s.format)),
                    byte_range: s.byte_range,
                })
                .collect(),
        }
    }

    /// Append anything that implements [`SyntaxHighlighting`].
    #[inline]
    pub fn with(mut self, portion: &dyn SyntaxHighlighting) -> Self {
        portion.syntax_highlight_into(&mut self);
        self
    }

    /// Append anything that implements [`SyntaxHighlighting`].
    #[inline]
    pub fn append(&mut self, portion: &dyn SyntaxHighlighting) -> &mut Self {
        portion.syntax_highlight_into(self);
        self
    }

    fn append_kind(&mut self, style: SyntaxHighlightedStyle, portion: &str) -> &mut Self {
        let start = self.text.len();
        self.text.push_str(portion);
        let end = self.text.len();
        self.parts.push(SyntaxHighlightedPart {
            byte_range: start..end,
            style,
        });
        self
    }
}

macro_rules! impl_style_fns {
    ($docs:literal, $pure:ident, $with:ident, $append:ident, $style:ident) => {
        impl_style_fns!($docs, $pure, $with, $append, (self, portion) {
            self.append_kind(SyntaxHighlightedStyle::$style, portion);
        });
    };
    ($docs:literal, $pure:ident, $with:ident, $append:ident, ($self:ident, $portion:ident) $content:expr) => {
        #[doc = $docs]
        #[inline]
        pub fn $with(mut self, portion: &str) -> Self {
            self.$append(portion);
            self
        }

        #[doc = $docs]
        #[inline]
        pub fn $append(&mut $self, $portion: &str) -> &mut Self {
            $content
            $self
        }

        #[doc = $docs]
        #[inline]
        pub fn $pure(portion: &str) -> Self {
            Self::new().$with(portion)
        }
    };
}

impl SyntaxHighlightedBuilder {
    impl_style_fns!("null", null, with_null, append_null, Null);

    impl_style_fns!(
        "Some primitive value, e.g. a number or bool.",
        primitive,
        with_primitive,
        append_primitive,
        Primitive
    );

    impl_style_fns!(
        "A string identifier.\n\nE.g. a variable name, field name, etc. Won't be quoted.",
        identifier,
        with_identifier,
        append_identifier,
        Identifier
    );

    impl_style_fns!(
        "Some string data. Will be quoted.",
        string_value,
        with_string_value,
        append_string_value,
        (self, portion) {
            let quote = Self::QUOTE_CHAR.to_string();
            self.append_kind(SyntaxHighlightedStyle::StringValue, &quote);
            self.append_kind(SyntaxHighlightedStyle::StringValue, portion);
            self.append_kind(SyntaxHighlightedStyle::StringValue, &quote);
        }
    );

    impl_style_fns!(
        "A keyword, e.g. a filter operator, like `and` or `all`",
        keyword,
        with_keyword,
        append_keyword,
        Keyword
    );

    impl_style_fns!(
        "An index number, e.g. an array index.",
        index,
        with_index,
        append_index,
        Index
    );

    impl_style_fns!(
        "Some syntax, e.g. brackets, commas, colons, etc.",
        syntax,
        with_syntax,
        append_syntax,
        Syntax
    );

    impl_style_fns!(
        "Body text, subdued (default label color).",
        body,
        with_body,
        append_body,
        Body
    );

    impl_style_fns!(
        "Body text with default color (color of inactive buttons).",
        body_default,
        with_body_default,
        append_body_default,
        BodyDefault
    );

    impl_style_fns!(
        "Body text in italics, e.g. for emphasis.",
        body_italics,
        with_body_italics,
        append_body_italics,
        BodyItalics
    );

    /// Append text with a custom format.
    #[inline]
    pub fn append_with_format(&mut self, text: &str, format: TextFormat) -> &mut Self {
        self.append_kind(SyntaxHighlightedStyle::Custom(Box::new(format)), text);
        self
    }

    /// Append text with a custom format closure.
    #[inline]
    pub fn append_with_format_closure<F>(&mut self, text: &str, f: F) -> &mut Self
    where
        F: 'static + Fn(&Style) -> TextFormat,
    {
        self.append_kind(SyntaxHighlightedStyle::CustomClosure(Box::new(f)), text);
        self
    }

    /// With a custom format.
    #[inline]
    pub fn with_format(mut self, text: &str, format: TextFormat) -> Self {
        self.append_with_format(text, format);
        self
    }

    /// With a custom format closure.
    #[inline]
    pub fn with_format_closure<F>(mut self, text: &str, f: F) -> Self
    where
        F: 'static + Fn(&Style) -> TextFormat,
    {
        self.append_with_format_closure(text, f);
        self
    }
}

// ----------------------------------------------------------------------------

impl SyntaxHighlightedBuilder {
    #[inline]
    pub fn into_job(self, style: &Style) -> LayoutJob {
        let mut job = LayoutJob {
            text: self.text,
            sections: Vec::with_capacity(self.parts.len()),
            ..Default::default()
        };

        for part in self.parts {
            let format = part.style.into_format(style);
            job.sections.push(egui::text::LayoutSection {
                byte_range: part.byte_range,
                format,
                leading_space: 0.0,
            });
        }

        job
    }

    #[inline]
    pub fn into_widget_text(self, style: &Style) -> egui::WidgetText {
        self.into_job(style).into()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

// ----------------------------------------------------------------------------

enum SyntaxHighlightedStyle {
    StringValue,
    Identifier,
    Keyword,
    Index,
    Null,
    Primitive,
    Syntax,
    Body,
    BodyDefault,
    BodyItalics,
    Custom(Box<TextFormat>),
    CustomClosure(Box<dyn Fn(&Style) -> TextFormat>),
}

impl std::fmt::Debug for SyntaxHighlightedStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StringValue => write!(f, "StringValue"),
            Self::Identifier => write!(f, "Identifier"),
            Self::Keyword => write!(f, "Keyword"),
            Self::Index => write!(f, "Index"),
            Self::Null => write!(f, "Null"),
            Self::Primitive => write!(f, "Primitive"),
            Self::Syntax => write!(f, "Syntax"),
            Self::Body => write!(f, "Body"),
            Self::BodyDefault => write!(f, "BodyDefault"),
            Self::BodyItalics => write!(f, "BodyItalics"),
            Self::Custom(_) => write!(f, "Custom(…)"),
            Self::CustomClosure(_) => write!(f, "CustomClosure(…)"),
        }
    }
}

#[derive(Debug)]
struct SyntaxHighlightedPart {
    byte_range: std::ops::Range<usize>,
    style: SyntaxHighlightedStyle,
}

impl SyntaxHighlightedStyle {
    /// Monospace text format with a specific color (that may be overridden by the style).
    pub fn monospace_with_color(style: &Style, color: Color32) -> TextFormat {
        TextFormat {
            font_id: TextStyle::Monospace.resolve(style),
            color: style.visuals.override_text_color.unwrap_or(color),
            ..Default::default()
        }
    }

    pub fn body_with_color(style: &Style, color: Color32) -> TextFormat {
        TextFormat {
            font_id: TextStyle::Body.resolve(style),
            color: style.visuals.override_text_color.unwrap_or(color),
            ..Default::default()
        }
    }

    pub fn body(style: &Style) -> TextFormat {
        Self::body_with_color(style, Color32::PLACEHOLDER)
    }

    pub fn into_format(self, style: &Style) -> TextFormat {
        match self {
            Self::StringValue => {
                Self::monospace_with_color(style, style.tokens().code_string_color)
            }
            Self::Identifier => Self::monospace_with_color(style, style.tokens().text_default),
            // TODO(lucas): Find a better way to deal with body / monospace style
            Self::Keyword => Self::body_with_color(style, style.tokens().code_keyword_color),
            Self::Index => Self::monospace_with_color(style, style.tokens().code_index_color),
            Self::Null => Self::monospace_with_color(style, style.tokens().code_null_color),
            Self::Primitive => {
                Self::monospace_with_color(style, style.tokens().code_primitive_color)
            }
            Self::Syntax => Self::monospace_with_color(style, style.tokens().text_subdued),
            Self::Body => Self::body(style),
            Self::BodyDefault => {
                let mut format = Self::body(style);
                format.color = style
                    .visuals
                    .override_text_color
                    .unwrap_or_else(|| style.tokens().text_default);
                format
            }
            Self::BodyItalics => {
                let mut format = Self::body(style);
                format.italics = true;
                format
            }
            Self::Custom(format) => *format,
            Self::CustomClosure(f) => f(style),
        }
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
        builder.append_identifier(self.short_name());
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
            component,
        } = self;
        builder
            .append(entity_path)
            .append_syntax(":")
            .append(component);
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
        let mut builder = Self::new();
        portion.syntax_highlight_into(&mut builder);
        builder
    }
}
