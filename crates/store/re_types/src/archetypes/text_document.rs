// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/text_document.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: A text element intended to be displayed in its own text box.
///
/// Supports raw text and markdown.
///
/// ## Example
///
/// ### Markdown text document
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_document").spawn()?;
///
///     rec.log(
///         "text_document",
///         &rerun::TextDocument::new("Hello, TextDocument!"),
///     )?;
///
///     rec.log(
///         "markdown",
///         &rerun::TextDocument::from_markdown(
///             r#"
/// # Hello Markdown!
/// [Click here to see the raw text](recording://markdown:Text).
///
/// Basic formatting:
///
/// | **Feature**       | **Alternative** |
/// | ----------------- | --------------- |
/// | Plain             |                 |
/// | *italics*         | _italics_       |
/// | **bold**          | __bold__        |
/// | ~~strikethrough~~ |                 |
/// | `inline code`     |                 |
///
/// ----------------------------------
///
/// ## Support
/// - [x] [Commonmark](https://commonmark.org/help/) support
/// - [x] GitHub-style strikethrough, tables, and checkboxes
/// - Basic syntax highlighting for:
///   - [x] C and C++
///   - [x] Python
///   - [x] Rust
///   - [ ] Other languages
///
/// ## Links
/// You can link to [an entity](recording://markdown),
/// a [specific instance of an entity](recording://markdown[#0]),
/// or a [specific component](recording://markdown:Text).
///
/// Of course you can also have [normal https links](https://github.com/rerun-io/rerun), e.g. <https://rerun.io>.
///
/// ## Image
/// ![A random image](https://picsum.photos/640/480)
/// "#.trim(),
///         )
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/1200w.png">
///   <img src="https://static.rerun.io/textdocument/babda19558ee32ed8d730495b595aee7a5e2c174/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDocument {
    /// Contents of the text document.
    pub text: crate::components::Text,

    /// The Media Type of the text.
    ///
    /// For instance:
    /// * `text/plain`
    /// * `text/markdown`
    ///
    /// If omitted, `text/plain` is assumed.
    pub media_type: Option<crate::components::MediaType>,
}

impl ::re_types_core::SizeBytes for TextDocument {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.text.heap_size_bytes() + self.media_type.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::Text>::is_pod() && <Option<crate::components::MediaType>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Text".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TextDocumentIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.MediaType".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Text".into(),
            "rerun.components.TextDocumentIndicator".into(),
            "rerun.components.MediaType".into(),
        ]
    });

impl TextDocument {
    /// The total number of components in the archetype: 1 required, 1 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`TextDocument`] [`::re_types_core::Archetype`]
pub type TextDocumentIndicator = ::re_types_core::GenericIndicatorComponent<TextDocument>;

impl ::re_types_core::Archetype for TextDocument {
    type Indicator = TextDocumentIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.TextDocument".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Text document"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: TextDocumentIndicator = TextDocumentIndicator::DEFAULT;
        MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let text = {
            let array = arrays_by_name
                .get("rerun.components.Text")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextDocument#text")?;
            <crate::components::Text>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.TextDocument#text")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextDocument#text")?
        };
        let media_type = if let Some(array) = arrays_by_name.get("rerun.components.MediaType") {
            <crate::components::MediaType>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.TextDocument#media_type")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { text, media_type })
    }
}

impl ::re_types_core::AsComponents for TextDocument {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.text as &dyn ComponentBatch).into()),
            self.media_type
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl TextDocument {
    /// Create a new `TextDocument`.
    #[inline]
    pub fn new(text: impl Into<crate::components::Text>) -> Self {
        Self {
            text: text.into(),
            media_type: None,
        }
    }

    /// The Media Type of the text.
    ///
    /// For instance:
    /// * `text/plain`
    /// * `text/markdown`
    ///
    /// If omitted, `text/plain` is assumed.
    #[inline]
    pub fn with_media_type(mut self, media_type: impl Into<crate::components::MediaType>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }
}
