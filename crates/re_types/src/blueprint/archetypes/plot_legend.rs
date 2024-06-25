// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/plot_legend.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
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

/// **Archetype**: Configuration for the legend of a plot.
#[derive(Clone, Debug, Default)]
pub struct PlotLegend {
    /// To what corner the legend is aligned.
    ///
    /// Defaults to the right bottom corner.
    pub corner: Option<crate::blueprint::components::Corner2D>,

    /// Whether the legend is shown at all.
    ///
    /// True by default.
    pub visible: Option<crate::blueprint::components::Visible>,
}

impl ::re_types_core::SizeBytes for PlotLegend {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.corner.heap_size_bytes() + self.visible.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::blueprint::components::Corner2D>>::is_pod()
            && <Option<crate::blueprint::components::Visible>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.blueprint.components.PlotLegendIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.Corner2D".into(),
            "rerun.blueprint.components.Visible".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.blueprint.components.PlotLegendIndicator".into(),
            "rerun.blueprint.components.Corner2D".into(),
            "rerun.blueprint.components.Visible".into(),
        ]
    });

impl PlotLegend {
    /// The total number of components in the archetype: 0 required, 1 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`PlotLegend`] [`::re_types_core::Archetype`]
pub type PlotLegendIndicator = ::re_types_core::GenericIndicatorComponent<PlotLegend>;

impl ::re_types_core::Archetype for PlotLegend {
    type Indicator = PlotLegendIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.PlotLegend".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Plot legend"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: PlotLegendIndicator = PlotLegendIndicator::DEFAULT;
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
        let corner = if let Some(array) = arrays_by_name.get("rerun.blueprint.components.Corner2D")
        {
            <crate::blueprint::components::Corner2D>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.PlotLegend#corner")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let visible = if let Some(array) = arrays_by_name.get("rerun.blueprint.components.Visible")
        {
            <crate::blueprint::components::Visible>::from_arrow_opt(&**array)
                .with_context("rerun.blueprint.archetypes.PlotLegend#visible")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { corner, visible })
    }
}

impl ::re_types_core::AsComponents for PlotLegend {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.corner
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.visible
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for PlotLegend {}

impl PlotLegend {
    /// Create a new `PlotLegend`.
    #[inline]
    pub fn new() -> Self {
        Self {
            corner: None,
            visible: None,
        }
    }

    /// To what corner the legend is aligned.
    ///
    /// Defaults to the right bottom corner.
    #[inline]
    pub fn with_corner(
        mut self,
        corner: impl Into<crate::blueprint::components::Corner2D>,
    ) -> Self {
        self.corner = Some(corner.into());
        self
    }

    /// Whether the legend is shown at all.
    ///
    /// True by default.
    #[inline]
    pub fn with_visible(
        mut self,
        visible: impl Into<crate::blueprint::components::Visible>,
    ) -> Self {
        self.visible = Some(visible.into());
        self
    }
}
