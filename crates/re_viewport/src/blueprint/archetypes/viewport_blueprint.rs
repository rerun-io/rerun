// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/viewport_blueprint.fbs".

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
pub struct ViewportBlueprint {
    /// All of the space-views that belong to the viewport.
    pub space_views: Vec<crate::blueprint::components::IncludedSpaceView>,

    /// The layout of the space-views
    pub root_container: Option<crate::blueprint::components::RootContainer>,

    /// Show one tab as maximized?
    pub maximized: Option<crate::blueprint::components::SpaceViewMaximized>,

    /// Whether the viewport layout is determined automatically.
    ///
    /// If `true`, the container layout will be reset whenever a new space view is added or removed.
    /// This defaults to `false` and is automatically set to `false` when there is user determined layout.
    pub auto_layout: Option<crate::blueprint::components::AutoLayout>,

    /// Whether or not space views should be created automatically.
    ///
    /// True if not specified, meaning that if the Viewer deems it necessary to add new Space Views to cover
    /// all logged entities appropriately, it will do so unless they were added previously
    /// (as identified by `past_viewer_recommendations`).
    pub auto_space_views: Option<crate::blueprint::components::AutoSpaceViews>,

    /// Hashes of all recommended space views the viewer has already added and that should not be added again.
    ///
    /// This is an internal field and should not be set usually.
    /// If you want the viewer from stopping to add space views, you should set `auto_space_views` to `false`.
    ///
    /// The viewer uses this to determine whether it should keep adding space views.
    pub past_viewer_recommendations:
        Option<Vec<crate::blueprint::components::ViewerRecommendationHash>>,
}

impl ::re_types_core::SizeBytes for ViewportBlueprint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.space_views.heap_size_bytes()
            + self.root_container.heap_size_bytes()
            + self.maximized.heap_size_bytes()
            + self.auto_layout.heap_size_bytes()
            + self.auto_space_views.heap_size_bytes()
            + self.past_viewer_recommendations.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::blueprint::components::IncludedSpaceView>>::is_pod()
            && <Option<crate::blueprint::components::RootContainer>>::is_pod()
            && <Option<crate::blueprint::components::SpaceViewMaximized>>::is_pod()
            && <Option<crate::blueprint::components::AutoLayout>>::is_pod()
            && <Option<crate::blueprint::components::AutoSpaceViews>>::is_pod()
            && <Option<Vec<crate::blueprint::components::ViewerRecommendationHash>>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.IncludedSpaceView".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.ViewportBlueprintIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 6usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.AutoLayout".into(),
            "rerun.blueprint.components.AutoSpaceViews".into(),
            "rerun.blueprint.components.RootContainer".into(),
            "rerun.blueprint.components.SpaceViewMaximized".into(),
            "rerun.blueprint.components.ViewerRecommendationHash".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 8usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.IncludedSpaceView".into(),
            "rerun.blueprint.components.ViewportBlueprintIndicator".into(),
            "rerun.blueprint.components.AutoLayout".into(),
            "rerun.blueprint.components.AutoSpaceViews".into(),
            "rerun.blueprint.components.RootContainer".into(),
            "rerun.blueprint.components.SpaceViewMaximized".into(),
            "rerun.blueprint.components.ViewerRecommendationHash".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl ViewportBlueprint {
    pub const NUM_COMPONENTS: usize = 8usize;
}

/// Indicator component for the [`ViewportBlueprint`] [`::re_types_core::Archetype`]
pub type ViewportBlueprintIndicator = ::re_types_core::GenericIndicatorComponent<ViewportBlueprint>;

impl ::re_types_core::Archetype for ViewportBlueprint {
    type Indicator = ViewportBlueprintIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.ViewportBlueprint".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: ViewportBlueprintIndicator = ViewportBlueprintIndicator::DEFAULT;
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
        let space_views = {
            let array = arrays_by_name
                .get("rerun.blueprint.components.IncludedSpaceView")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.blueprint.archetypes.ViewportBlueprint#space_views")?;
            <crate::blueprint::components::IncludedSpaceView>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.ViewportBlueprint#space_views")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.blueprint.archetypes.ViewportBlueprint#space_views")?
        };
        let root_container =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.RootContainer") {
                <crate::blueprint::components::RootContainer>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.ViewportBlueprint#root_container")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let maximized = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.SpaceViewMaximized")
        {
            <crate::blueprint::components::SpaceViewMaximized>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.ViewportBlueprint#maximized")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let auto_layout =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.AutoLayout") {
                <crate::blueprint::components::AutoLayout>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.ViewportBlueprint#auto_layout")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let auto_space_views =
            if let Some(array) = arrays_by_name.get("rerun.blueprint.components.AutoSpaceViews") {
                <crate::blueprint::components::AutoSpaceViews>::from_arrow_opt(&**array)
                    .with_context("rerun.blueprint.archetypes.ViewportBlueprint#auto_space_views")?
                    .into_iter()
                    .next()
                    .flatten()
            } else {
                None
            };
        let past_viewer_recommendations = if let Some(array) =
            arrays_by_name.get("rerun.blueprint.components.ViewerRecommendationHash")
        {
            Some({
                <crate::blueprint::components::ViewerRecommendationHash>::from_arrow_opt(&**array)
                    .with_context(
                        "rerun.blueprint.archetypes.ViewportBlueprint#past_viewer_recommendations",
                    )?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context(
                        "rerun.blueprint.archetypes.ViewportBlueprint#past_viewer_recommendations",
                    )?
            })
        } else {
            None
        };
        Ok(Self {
            space_views,
            root_container,
            maximized,
            auto_layout,
            auto_space_views,
            past_viewer_recommendations,
        })
    }
}

impl ::re_types_core::AsComponents for ViewportBlueprint {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.space_views as &dyn ComponentBatch).into()),
            self.root_container
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.maximized
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.auto_layout
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.auto_space_views
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.past_viewer_recommendations
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        self.space_views.len()
    }
}

impl ViewportBlueprint {
    pub fn new(
        space_views: impl IntoIterator<
            Item = impl Into<crate::blueprint::components::IncludedSpaceView>,
        >,
    ) -> Self {
        Self {
            space_views: space_views.into_iter().map(Into::into).collect(),
            root_container: None,
            maximized: None,
            auto_layout: None,
            auto_space_views: None,
            past_viewer_recommendations: None,
        }
    }

    #[inline]
    pub fn with_root_container(
        mut self,
        root_container: impl Into<crate::blueprint::components::RootContainer>,
    ) -> Self {
        self.root_container = Some(root_container.into());
        self
    }

    #[inline]
    pub fn with_maximized(
        mut self,
        maximized: impl Into<crate::blueprint::components::SpaceViewMaximized>,
    ) -> Self {
        self.maximized = Some(maximized.into());
        self
    }

    #[inline]
    pub fn with_auto_layout(
        mut self,
        auto_layout: impl Into<crate::blueprint::components::AutoLayout>,
    ) -> Self {
        self.auto_layout = Some(auto_layout.into());
        self
    }

    #[inline]
    pub fn with_auto_space_views(
        mut self,
        auto_space_views: impl Into<crate::blueprint::components::AutoSpaceViews>,
    ) -> Self {
        self.auto_space_views = Some(auto_space_views.into());
        self
    }

    #[inline]
    pub fn with_past_viewer_recommendations(
        mut self,
        past_viewer_recommendations: impl IntoIterator<
            Item = impl Into<crate::blueprint::components::ViewerRecommendationHash>,
        >,
    ) -> Self {
        self.past_viewer_recommendations = Some(
            past_viewer_recommendations
                .into_iter()
                .map(Into::into)
                .collect(),
        );
        self
    }
}
