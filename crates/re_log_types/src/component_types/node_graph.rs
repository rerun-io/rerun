use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::Component;

// ---

/// A double-precision NodeGraph.
///
/// ## Examples
///
/// ```
/// # use re_log_types::component_types::NodeGraph;
/// # use arrow2_convert::field::ArrowField;
/// # use arrow2::datatypes::{DataType, Field};
/// assert_eq!(NodeGraph::data_type(), DataType::Float64);
/// ```
#[derive(Debug, Clone, Copy, ArrowField, ArrowSerialize, ArrowDeserialize)]
#[arrow_field(transparent)]
pub struct NodeGraph(pub f64);

impl Component for NodeGraph {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.pipeline_graph".into()
    }
}

impl From<f64> for NodeGraph {
    #[inline]
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl From<NodeGraph> for f64 {
    #[inline]
    fn from(value: NodeGraph) -> Self {
        value.0
    }
}
