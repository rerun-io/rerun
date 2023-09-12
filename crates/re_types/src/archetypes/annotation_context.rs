// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs".

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

/// The `AnnotationContext` provides additional information on how to display entities.
///
/// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// `AnnotationContext`. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
///
/// ## Example
///
/// ```ignore
/// //! Log rectangles with different colors and labels using annotation context
///
/// use rerun::{
///    archetypes::{AnnotationContext, Boxes2D},
///    datatypes::Color,
///    RecordingStreamBuilder,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///    let (rec, storage) =
///        RecordingStreamBuilder::new("rerun_example_annotation_context_rects").memory()?;
///
///    // Log an annotation context to assign a label and color to each class
///    rec.log(
///        "/",
///        &AnnotationContext::new([
///            (1, "red", Color::from(0xFF0000FF)),
///            (2, "green", Color::from(0x00FF00FF)),
///        ]),
///    )?;
///
///    // Log a batch of 2 rectangles with different class IDs
///    rec.log(
///        "detections",
///        &Boxes2D::from_mins_and_sizes([(-2., -2.), (0., 0.)], [(3., 3.), (2., 2.)])
///            .with_class_ids([1, 2]),
///    )?;
///
///    // Log an extra rect to set the view bounds
///    rec.log(
///        "bounds",
///        &Boxes2D::from_mins_and_sizes([(0., 0.)], [(5., 5.)]),
///    )?;
///
///    rerun::native_viewer::show(storage.take())?;
///    Ok(())
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationContext {
    pub context: crate::components::AnnotationContext,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.annotation_context".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.AnnotationContextIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.annotation_context".into(),
            "rerun.components.AnnotationContextIndicator".into(),
        ]
    });

impl AnnotationContext {
    pub const NUM_COMPONENTS: usize = 2usize;
}

/// Indicator component for the [`AnnotationContext`] [`crate::Archetype`]
pub type AnnotationContextIndicator = crate::GenericIndicatorComponent<AnnotationContext>;

impl crate::Archetype for AnnotationContext {
    type Indicator = AnnotationContextIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.AnnotationContext".into()
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
            Some(Self::Indicator::batch(self.num_instances() as _).into()),
            Some((&self.context as &dyn crate::ComponentBatch).into()),
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
        Ok([{
            Some({
                let array = <crate::components::AnnotationContext>::try_to_arrow([&self.context]);
                array.map(|array| {
                    let datatype = ::arrow2::datatypes::DataType::Extension(
                        "rerun.components.AnnotationContext".into(),
                        Box::new(array.data_type().clone()),
                        Some("rerun.annotation_context".into()),
                    );
                    (
                        ::arrow2::datatypes::Field::new("context", datatype, false),
                        array,
                    )
                })
            })
            .transpose()
            .with_context("rerun.archetypes.AnnotationContext#context")?
        }]
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
        let context = {
            let array = arrays_by_name
                .get("context")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?;
            <crate::components::AnnotationContext>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.AnnotationContext#context")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.AnnotationContext#context")?
        };
        Ok(Self { context })
    }
}

impl AnnotationContext {
    pub fn new(context: impl Into<crate::components::AnnotationContext>) -> Self {
        Self {
            context: context.into(),
        }
    }
}
