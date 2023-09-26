// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

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

/// A log entry in a text log, comprised of a text body and its log level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLog {
    pub body: crate::components::Text,
    pub level: Option<crate::components::TextLogLevel>,
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
    fn num_instances(&self) -> usize {
        1
    }

    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        [
            Some(Self::indicator().into()),
            Some((&self.body as &dyn crate::ComponentBatch).into()),
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
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        use crate::{Loggable as _, ResultExt as _};
        Ok([
            {
                Some({
                    let array = <crate::components::Text>::try_to_arrow([&self.body]);
                    array.map(|array| {
                        let datatype = ::arrow2::datatypes::DataType::Extension(
                            "rerun.components.Text".into(),
                            Box::new(array.data_type().clone()),
                            None,
                        );
                        (
                            ::arrow2::datatypes::Field::new("body", datatype, false),
                            array,
                        )
                    })
                })
                .transpose()
                .with_context("rerun.archetypes.TextLog#body")?
            },
            {
                self.level
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::TextLogLevel>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.TextLogLevel".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("level", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.TextLog#level")?
            },
            {
                self.color
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::Color>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.Color".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("color", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.TextLog#color")?
            },
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    #[inline]
    fn try_from_arrow(
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
                .with_context("rerun.archetypes.TextLog#body")?;
            <crate::components::Text>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.TextLog#body")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TextLog#body")?
        };
        let level = if let Some(array) = arrays_by_name.get("level") {
            Some({
                <crate::components::TextLogLevel>::try_from_arrow_opt(&**array)
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
        let color = if let Some(array) = arrays_by_name.get("color") {
            Some({
                <crate::components::Color>::try_from_arrow_opt(&**array)
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
        Ok(Self { body, level, color })
    }
}

impl TextLog {
    pub fn new(body: impl Into<crate::components::Text>) -> Self {
        Self {
            body: body.into(),
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
