// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/graph_edges.fbs".

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

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: A list of edges in a graph.
///
/// By default, edges are undirected.
///
/// ⚠️ **This type is experimental and may be removed in future versions**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphEdges {
    /// A list of node IDs.
    pub edges: Vec<crate::components::GraphEdge>,

    /// Specifies if the graph is directed or undirected.
    ///
    /// If no `GraphType` is provided, the graph is assumed to be undirected.
    pub graph_type: Option<crate::components::GraphType>,
}

impl ::re_types_core::SizeBytes for GraphEdges {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.edges.heap_size_bytes() + self.graph_type.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<crate::components::GraphEdge>>::is_pod()
            && <Option<crate::components::GraphType>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.GraphEdge".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.GraphType".into(),
            "rerun.components.GraphEdgesIndicator".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.GraphEdge".into(),
            "rerun.components.GraphType".into(),
            "rerun.components.GraphEdgesIndicator".into(),
        ]
    });

impl GraphEdges {
    /// The total number of components in the archetype: 1 required, 2 recommended, 0 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`GraphEdges`] [`::re_types_core::Archetype`]
pub type GraphEdgesIndicator = ::re_types_core::GenericIndicatorComponent<GraphEdges>;

impl ::re_types_core::Archetype for GraphEdges {
    type Indicator = GraphEdgesIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.GraphEdges".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Graph edges"
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: GraphEdgesIndicator = GraphEdgesIndicator::DEFAULT;
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
        let edges = {
            let array = arrays_by_name
                .get("rerun.components.GraphEdge")
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.GraphEdges#edges")?;
            <crate::components::GraphEdge>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.GraphEdges#edges")?
                .into_iter()
                .map(|v| v.ok_or_else(DeserializationError::missing_data))
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.archetypes.GraphEdges#edges")?
        };
        let graph_type = if let Some(array) = arrays_by_name.get("rerun.components.GraphType") {
            <crate::components::GraphType>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.GraphEdges#graph_type")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { edges, graph_type })
    }
}

impl ::re_types_core::AsComponents for GraphEdges {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.edges as &dyn ComponentBatch).into()),
            self.graph_type
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for GraphEdges {}

impl GraphEdges {
    /// Create a new `GraphEdges`.
    #[inline]
    pub fn new(edges: impl IntoIterator<Item = impl Into<crate::components::GraphEdge>>) -> Self {
        Self {
            edges: edges.into_iter().map(Into::into).collect(),
            graph_type: None,
        }
    }

    /// Specifies if the graph is directed or undirected.
    ///
    /// If no `GraphType` is provided, the graph is assumed to be undirected.
    #[inline]
    pub fn with_graph_type(mut self, graph_type: impl Into<crate::components::GraphType>) -> Self {
        self.graph_type = Some(graph_type.into());
        self
    }
}
