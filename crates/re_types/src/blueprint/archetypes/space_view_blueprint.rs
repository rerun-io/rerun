// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/space_view_blueprint.fbs".

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
pub struct SpaceViewBlueprint {
    /// The class of the view.
    pub class_identifier: crate::blueprint::components::SpaceViewClass,

    /// The name of the view.
    pub display_name: Option<crate::blueprint::components::Name>,

    /// The "anchor point" of this space view.
    ///
    /// The transform at this path forms the reference point for all scene->world transforms in this space view.
    /// I.e. the position of this entity path in space forms the origin of the coordinate system in this space view.
    /// Furthermore, this is the primary indicator for heuristics on what entities we show in this space view.
    pub space_origin: Option<crate::blueprint::components::SpaceViewOrigin>,

    /// True if the user is has added entities themselves. False otherwise.
    pub entities_determined_by_user: Option<crate::blueprint::components::EntitiesDeterminedByUser>,

    /// `BlueprintId`s of the `DataQuery`s that make up this `SpaceView`.
    ///
    /// It determines which entities are part of the spaceview.
    pub contents: Option<crate::blueprint::components::IncludedQueries>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.SpaceViewClass".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(
        || ["rerun.blueprint.components.SpaceViewBlueprintIndicator".into()],
    );

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.EntitiesDeterminedByUser".into(),
            "rerun.blueprint.components.IncludedQueries".into(),
            "rerun.blueprint.components.Name".into(),
            "rerun.blueprint.components.SpaceViewOrigin".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 7usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.SpaceViewClass".into(),
            "rerun.blueprint.components.SpaceViewBlueprintIndicator".into(),
            "rerun.blueprint.components.EntitiesDeterminedByUser".into(),
            "rerun.blueprint.components.IncludedQueries".into(),
            "rerun.blueprint.components.Name".into(),
            "rerun.blueprint.components.SpaceViewOrigin".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl SpaceViewBlueprint {
    pub const NUM_COMPONENTS: usize = 7usize;
}

/// Indicator component for the [`SpaceViewBlueprint`] [`::re_types_core::Archetype`]
pub type SpaceViewBlueprintIndicator =
    ::re_types_core::GenericIndicatorComponent<SpaceViewBlueprint>;

impl ::re_types_core::Archetype for SpaceViewBlueprint {
    type Indicator = SpaceViewBlueprintIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.SpaceViewBlueprint".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: SpaceViewBlueprintIndicator = SpaceViewBlueprintIndicator::DEFAULT;
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
        let class_identifier = {
            let array = arrays_by_name
                .get("rerun.blueprint.components.SpaceViewClass")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.SpaceViewBlueprint#class_identifier")?;
            <crate::blueprint::components::SpaceViewClass>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.SpaceViewBlueprint#class_identifier")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.SpaceViewBlueprint#class_identifier")?
        };
        let display_name =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.Name") {
                <crate::blueprint::components::Name>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.SpaceViewBlueprint#display_name")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let space_origin =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.SpaceViewOrigin") {
                <crate::blueprint::components::SpaceViewOrigin>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.SpaceViewBlueprint#space_origin")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let entities_determined_by_user = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.EntitiesDeterminedByUser")
        {
            <crate::blueprint::components::EntitiesDeterminedByUser>::from_arrow_opt(&**array)
                .with_context(
                    "rerun.blueprint.archetypes.SpaceViewBlueprint#entities_determined_by_user",
                )?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let contents =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.IncludedQueries") {
                <crate::blueprint::components::IncludedQueries>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.SpaceViewBlueprint#contents")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        Ok(Self {
            class_identifier,
            display_name,
            space_origin,
            entities_determined_by_user,
            contents,
        })
    }
}

impl ::re_types_core::AsComponents for SpaceViewBlueprint {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.class_identifier as &dyn ComponentBatch).into()),
            self.display_name
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.space_origin
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.entities_determined_by_user
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.contents
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

impl SpaceViewBlueprint {
    pub fn new(class_identifier: impl Into<crate::blueprint::components::SpaceViewClass>) -> Self {
        Self {
            class_identifier: class_identifier.into(),
            display_name: None,
            space_origin: None,
            entities_determined_by_user: None,
            contents: None,
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
    pub fn with_space_origin(
        mut self,
        space_origin: impl Into<crate::blueprint::components::SpaceViewOrigin>,
    ) -> Self {
        self.space_origin = Some(space_origin.into());
        self
    }

    #[inline]
    pub fn with_entities_determined_by_user(
        mut self,
        entities_determined_by_user: impl Into<crate::blueprint::components::EntitiesDeterminedByUser>,
    ) -> Self {
        self.entities_determined_by_user = Some(entities_determined_by_user.into());
        self
    }

    #[inline]
    pub fn with_contents(
        mut self,
        contents: impl Into<crate::blueprint::components::IncludedQueries>,
    ) -> Self {
        self.contents = Some(contents.into());
        self
    }
}
