use re_data_store::StoreSubscriber;

/// Topology information for a spatial view.
///
/// Describes how 2D & 3D spaces are connected/disconnected.
///
/// Used to determine whether 2D/3D visualizers are applicable and to inform
/// space view generation heuristics.
///
/// Spatial topology is time independent but may change as new data comes in.
/// Generally, the assumption is that topological cuts stay constant over time.
struct SpatialTopology {}

impl StoreSubscriber for SpatialTopology {
    fn name(&self) -> String {
        "SpatialTopology".to_owned()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[re_data_store::StoreEvent]) {
        todo!()
    }
}
