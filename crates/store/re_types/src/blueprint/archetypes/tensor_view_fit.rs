// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_view_fit.fbs".

#![allow(unused_braces)]
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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch as _, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentType};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Configures how a selected tensor slice is shown on screen.
///
/// ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
#[derive(Clone, Debug, Default)]
pub struct TensorViewFit {
    /// How the image is scaled to fit the view.
    pub scaling: Option<SerializedComponentBatch>,
}

impl TensorViewFit {
    /// Returns the [`ComponentDescriptor`] for [`Self::scaling`].
    ///
    /// The corresponding component is [`crate::blueprint::components::ViewFit`].
    #[inline]
    pub fn descriptor_scaling() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: Some("rerun.blueprint.archetypes.TensorViewFit".into()),
            component: "TensorViewFit:scaling".into(),
            component_type: Some("rerun.blueprint.components.ViewFit".into()),
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [TensorViewFit::descriptor_scaling()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [TensorViewFit::descriptor_scaling()]);

impl TensorViewFit {
    /// The total number of components in the archetype: 0 required, 0 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 1usize;
}

impl ::re_types_core::Archetype for TensorViewFit {
    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.TensorViewFit".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Tensor view fit"
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentDescriptor, arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_descr: ::nohash_hasher::IntMap<_, _> = arrow_data.into_iter().collect();
        let scaling = arrays_by_descr
            .get(&Self::descriptor_scaling())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_scaling()));
        Ok(Self { scaling })
    }
}

impl ::re_types_core::AsComponents for TensorViewFit {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        std::iter::once(self.scaling.clone()).flatten().collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for TensorViewFit {}

impl TensorViewFit {
    /// Create a new `TensorViewFit`.
    #[inline]
    pub fn new() -> Self {
        Self { scaling: None }
    }

    /// Update only some specific fields of a `TensorViewFit`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `TensorViewFit`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            scaling: Some(SerializedComponentBatch::new(
                crate::blueprint::components::ViewFit::arrow_empty(),
                Self::descriptor_scaling(),
            )),
        }
    }

    /// How the image is scaled to fit the view.
    #[inline]
    pub fn with_scaling(
        mut self,
        scaling: impl Into<crate::blueprint::components::ViewFit>,
    ) -> Self {
        self.scaling = try_serialize_field(Self::descriptor_scaling(), [scaling]);
        self
    }
}

impl ::re_byte_size::SizeBytes for TensorViewFit {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.scaling.heap_size_bytes()
    }
}
