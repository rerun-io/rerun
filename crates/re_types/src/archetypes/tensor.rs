// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/tensor.fbs".

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

/// **Archetype**: A generic n-dimensional Tensor.
///
/// ## Example
///
/// ### Simple Tensor
/// ```ignore
/// //! Create and log a tensor.
///
/// use ndarray::{Array, ShapeBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         rerun::RecordingStreamBuilder::new("rerun_example_tensor_simple").memory()?;
///
///     let mut data = Array::<u8, _>::default((8, 6, 3, 5).f());
///     data.map_inplace(|x| *x = rand::random());
///
///     let tensor = rerun::Tensor::try_from(data)?.with_names(["batch", "channel", "height", "width"]);
///     rec.log("tensor", &tensor)?;
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/1200w.png">
///   <img src="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct Tensor {
    /// The tensor data
    pub data: crate::components::TensorData,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TensorData".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TensorIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TensorData".into(),
            "rerun.components.TensorIndicator".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl Tensor {
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`Tensor`] [`::re_types_core::Archetype`]
pub type TensorIndicator = ::re_types_core::GenericIndicatorComponent<Tensor>;

impl ::re_types_core::Archetype for Tensor {
    type Indicator = TensorIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.Tensor".into()
    }

    #[inline]
    fn indicator() -> ::re_types_core::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: TensorIndicator = TensorIndicator::DEFAULT;
        ::re_types_core::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> ::re_types_core::DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let data = {
            let array = arrays_by_name
                .get("rerun.components.TensorData")
                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Tensor#data")?;
            <crate::components::TensorData>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.Tensor#data")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                .with_context("rerun.archetypes.Tensor#data")?
        };
        Ok(Self { data })
    }
}

impl ::re_types_core::AsComponents for Tensor {
    fn as_component_batches(&self) -> Vec<::re_types_core::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.data as &dyn ::re_types_core::ComponentBatch).into()),
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

impl Tensor {
    pub fn new(data: impl Into<crate::components::TensorData>) -> Self {
        Self { data: data.into() }
    }
}
