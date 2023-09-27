// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_document.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// A text element intended to be displayed in its own text-box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDocument {
    /// Contents of the text document.
    pub body: crate::components::Text,

    /// The Media Type of the text.
    ///
    /// For instance:
    /// * `text/plain`
    /// * `text/markdown`
    ///
    /// If omitted, `text/plain` is assumed.
    pub media_type: Option<crate::components::MediaType>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Text".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TextDocumentIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.InstanceKey".into(),
            "rerun.components.MediaType".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Text".into(),
            "rerun.components.TextDocumentIndicator".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.MediaType".into(),
        ]
    });

impl TextDocument {
    pub const NUM_COMPONENTS: usize = 4usize;
}

/// Indicator component for the [`TextDocument`] [`crate::Archetype`]
pub type TextDocumentIndicator = crate::GenericIndicatorComponent<TextDocument>;

impl crate::Archetype for TextDocument {
    type Indicator = TextDocumentIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.TextDocument".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: TextDocumentIndicator = TextDocumentIndicator::DEFAULT;
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
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let body = {
            let array = arrays_by_name
                .get("body")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextDocument#body")?;
            <crate::components::Text>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.TextDocument#body")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextDocument#body")?
        };
        let media_type = if let Some(array) = arrays_by_name.get("media_type") {
            Some({
                <crate::components::MediaType>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TextDocument#media_type")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TextDocument#media_type")?
            })
        } else {
            None
        };
        Ok(Self { body, media_type })
    }
}

impl crate::AsComponents for TextDocument {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.body as &dyn crate::ComponentBatch).into()),
            self.media_type
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

impl TextDocument {
    pub fn new(body: impl Into<crate::components::Text>) -> Self {
        Self {
            body: body.into(),
            media_type: None,
        }
    }

    pub fn with_media_type(mut self, media_type: impl Into<crate::components::MediaType>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }
}
