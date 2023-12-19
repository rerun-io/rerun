// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/container_blueprint.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
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

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: The top-level description of the Viewport.
#[derive(Clone, Debug, Default)]
pub struct ContainerBlueprint {
    /// The class of the view.
    pub container_kind: crate::blueprint::components::ContainerKind,

    /// The name of the container.
    pub display_name: Option<crate::blueprint::components::Name>,

    /// `ContainerIds`s or `SpaceViewId`s that are children of this container.
    pub contents: Option<crate::blueprint::components::IncludedContents>,

    /// The weights of the primary axis. For `Grid` this is the column weights.
    ///
    /// For `Horizontal`/`Vertical` containers, the length of this list should always match the number of contents.
    pub primary_weights: Option<crate::blueprint::components::PrimaryWeights>,

    /// The weights of the secondary axis. For `Grid` this is the row weights. Ignored for `Horizontal`/`Vertical` containers.
    pub secondary_weights: Option<crate::blueprint::components::SecondaryWeights>,

    /// Which tab is active.
    ///
    /// Only applies to `Tabs` containers.
    pub active_tab: Option<crate::blueprint::components::ActiveTab>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.ContainerKind".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(
        || ["rerun.blueprint.components.ContainerBlueprintIndicator".into()],
    );

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 6usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.ActiveTab".into(),
            "rerun.blueprint.components.IncludedContents".into(),
            "rerun.blueprint.components.Name".into(),
            "rerun.blueprint.components.PrimaryWeights".into(),
            "rerun.blueprint.components.SecondaryWeights".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 8usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.ContainerKind".into(),
            "rerun.blueprint.components.ContainerBlueprintIndicator".into(),
            "rerun.blueprint.components.ActiveTab".into(),
            "rerun.blueprint.components.IncludedContents".into(),
            "rerun.blueprint.components.Name".into(),
            "rerun.blueprint.components.PrimaryWeights".into(),
            "rerun.blueprint.components.SecondaryWeights".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl ContainerBlueprint {
    pub const NUM_COMPONENTS: usize = 8usize;
}

/// Indicator component for the [`ContainerBlueprint`] [`::re_types_core::Archetype`]
pub type ContainerBlueprintIndicator =
    ::re_types_core::GenericIndicatorComponent<ContainerBlueprint>;

impl ::re_types_core::Archetype for ContainerBlueprint {
    type Indicator = ContainerBlueprintIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.ContainerBlueprint".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: ContainerBlueprintIndicator = ContainerBlueprintIndicator::DEFAULT;
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
        let container_kind = {
            let array = arrays_by_name
                .get("rerun.blueprint.components.ContainerKind")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.ContainerBlueprint#container_kind")?;
            <crate::blueprint::components::ContainerKind>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.ContainerBlueprint#container_kind")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.ContainerBlueprint#container_kind")?
        };
        let display_name =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.Name") {
                <crate::blueprint::components::Name>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.ContainerBlueprint#display_name")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let contents = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.IncludedContents")
        {
            <crate::blueprint::components::IncludedContents>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.ContainerBlueprint#contents")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let primary_weights =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.PrimaryWeights") {
                <crate::blueprint::components::PrimaryWeights>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.ContainerBlueprint#primary_weights")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let secondary_weights = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.SecondaryWeights")
        {
            <crate::blueprint::components::SecondaryWeights>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.ContainerBlueprint#secondary_weights")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let active_tab =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.ActiveTab") {
                <crate::blueprint::components::ActiveTab>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.ContainerBlueprint#active_tab")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        Ok(Self {
            container_kind,
            display_name,
            contents,
            primary_weights,
            secondary_weights,
            active_tab,
        })
    }
}

impl ::re_types_core::AsComponents for ContainerBlueprint {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.container_kind as &dyn ComponentBatch).into()),
            self.display_name
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.contents
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.primary_weights
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.secondary_weights
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.active_tab
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
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

impl ContainerBlueprint {
    pub fn new(container_kind: impl Into<crate::blueprint::components::ContainerKind>) -> Self {
        Self {
            container_kind: container_kind.into(),
            display_name: None,
            contents: None,
            primary_weights: None,
            secondary_weights: None,
            active_tab: None,
        }
    }

    #[inline]
    pub fn with_display_name(
        mut self,
        display_name: impl Into<crate::blueprint::components::Name>,
    ) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    #[inline]
    pub fn with_contents(
        mut self,
        contents: impl Into<crate::blueprint::components::IncludedContents>,
    ) -> Self {
        self.contents = Some(contents.into());
        self
    }

    #[inline]
    pub fn with_primary_weights(
        mut self,
        primary_weights: impl Into<crate::blueprint::components::PrimaryWeights>,
    ) -> Self {
        self.primary_weights = Some(primary_weights.into());
        self
    }

    #[inline]
    pub fn with_secondary_weights(
        mut self,
        secondary_weights: impl Into<crate::blueprint::components::SecondaryWeights>,
    ) -> Self {
        self.secondary_weights = Some(secondary_weights.into());
        self
    }

    #[inline]
    pub fn with_active_tab(
        mut self,
        active_tab: impl Into<crate::blueprint::components::ActiveTab>,
    ) -> Self {
        self.active_tab = Some(active_tab.into());
        self
    }
}
