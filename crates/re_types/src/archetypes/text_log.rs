// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// **Archetype**: A log entry in a text log, comprised of a text body and its log level.
///
/// ## Example
///
/// ### `text_log_integration`:
/// ```ignore
/// //! Shows integration of Rerun's `TextLog` with the native logging interface.
///
/// use rerun::external::log;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_text_log_integration").memory()?;
///
///     // Log a text entry directly:
///     rec.log(
///         "logs",
///         &rerun::TextLog::new("this entry has loglevel TRACE")
///             .with_level(rerun::TextLogLevel::TRACE),
///     )?;
///
///     // Or log via a logging handler:
///     rerun::Logger::new(rec.clone()) // recording streams are ref-counted
///         .with_path_prefix("logs/handler")
///         // You can also use the standard `RUST_LOG` environment variable!
///         .with_filter(rerun::default_log_filter())
///         .init()?;
///     log::info!("This INFO log got added through the standard logging interface");
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/1200w.png">
///   <img src="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLog {
    /// The body of the message.
    pub text: crate::components::Text,

    /// The verbosity level of the message.
    ///
    /// This can be used to filter the log messages in the Rerun Viewer.
    pub level: Option<crate::components::TextLogLevel>,

    /// Optional color to use for the log line in the Rerun Viewer.
    pub color: Option<crate::components::Color>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Text".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TextLogIndicator".into(),
            "rerun.components.TextLogLevel".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Text".into(),
            "rerun.components.TextLogIndicator".into(),
            "rerun.components.TextLogLevel".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl TextLog {
    pub const NUM_COMPONENTS: usize = 4usize;
}

/// Indicator component for the [`TextLog`] [`crate::Archetype`]
pub type TextLogIndicator = crate::GenericIndicatorComponent<TextLog>;

impl crate::Archetype for TextLog {
    type Indicator = TextLogIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.TextLog".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: TextLogIndicator = TextLogIndicator::DEFAULT;
        crate::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        re_tracing::profile_function!();
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let text = {
            let array = arrays_by_name
                .get("rerun.components.Text")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextLog#text")?;
            <crate::components::Text>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.TextLog#text")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextLog#text")?
        };
        let level = if let Some(array) = arrays_by_name.get("rerun.components.TextLogLevel") {
            Some({
                <crate::components::TextLogLevel>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TextLog#level")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TextLog#level")?
            })
        } else {
            None
        };
        let color = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            Some({
                <crate::components::Color>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TextLog#color")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TextLog#color")?
            })
        } else {
            None
        };
        Ok(Self { text, level, color })
    }
}

impl crate::AsComponents for TextLog {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.text as &dyn crate::ComponentBatch).into()),
            self.level
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.color
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }
}

impl TextLog {
    pub fn new(text: impl Into<crate::components::Text>) -> Self {
        Self {
            text: text.into(),
            level: None,
            color: None,
        }
    }

    pub fn with_level(mut self, level: impl Into<crate::components::TextLogLevel>) -> Self {
        self.level = Some(level.into());
        self
    }

    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = Some(color.into());
        self
    }
}
