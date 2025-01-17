// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/panel_blueprint.fbs".

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
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Shared state for the 3 collapsible panels.
#[derive(Clone, Debug, Default)]
pub struct PanelBlueprint {
    /// Current state of the panels.
    pub state: Option<SerializedComponentBatch>,
}

impl PanelBlueprint {
    /// Returns the [`ComponentDescriptor`] for [`Self::state`].
    #[inline]
    pub fn descriptor_state() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.PanelBlueprint".into()),
            component_name: "rerun.blueprint.components.PanelState".into(),
            archetype_field_name: Some("state".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.PanelBlueprint".into()),
            component_name: "rerun.blueprint.components.PanelBlueprintIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [PanelBlueprint::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [PanelBlueprint::descriptor_state()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            PanelBlueprint::descriptor_indicator(),
            PanelBlueprint::descriptor_state(),
        ]
    });

impl PanelBlueprint {
    /// The total number of components in the archetype: 0 required, 1 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 2usize;
}

/// Indicator component for the [`PanelBlueprint`] [`::re_types_core::Archetype`]
pub type PanelBlueprintIndicator = ::re_types_core::GenericIndicatorComponent<PanelBlueprint>;

impl ::re_types_core::Archetype for PanelBlueprint {
    type Indicator = PanelBlueprintIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.PanelBlueprint".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Panel blueprint"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: PanelBlueprintIndicator = PanelBlueprintIndicator::DEFAULT;
        ComponentBatchCowWithDescriptor::new(&INDICATOR as &dyn ::re_types_core::ComponentBatch)
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
        let state = arrays_by_descr
            .get(&Self::descriptor_state())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_state()));
        Ok(Self { state })
    }
}

impl ::re_types_core::AsComponents for PanelBlueprint {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [Self::indicator().serialized(), self.state.clone()]
            .into_iter()
            .flatten()
            .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for PanelBlueprint {}

impl PanelBlueprint {
    /// Create a new `PanelBlueprint`.
    #[inline]
    pub fn new() -> Self {
        Self { state: None }
    }

    /// Update only some specific fields of a `PanelBlueprint`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `PanelBlueprint`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            state: Some(SerializedComponentBatch::new(
                crate::blueprint::components::PanelState::arrow_empty(),
                Self::descriptor_state(),
            )),
        }
    }

    /// Current state of the panels.
    #[inline]
    pub fn with_state(
        mut self,
        state: impl Into<crate::blueprint::components::PanelState>,
    ) -> Self {
        self.state = try_serialize_field(Self::descriptor_state(), [state]);
        self
    }
}

impl ::re_byte_size::SizeBytes for PanelBlueprint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.state.heap_size_bytes()
    }
}
