use re_entity_db::{InstancePath, InstancePathHash};
use re_log_types::EntityPathHash;
use re_space_view::{DataResultQuery as _, RangeResultsExt as _};
use re_types::{
    archetypes::GeoPoints,
    components::{Color, LatLon, Radius},
    Loggable as _,
};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, Item, ItemCollection, QueryContext,
    SpaceViewId, SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::visualizers::{update_picked_instance, PickedInstance};

#[derive(Debug, Clone)]
struct GeoPointEntry {
    /// Position.
    position: walkers::Position,

    /// Display radius in pixels
    //TODO(#7872): support for radius in meter
    radius: f32,

    /// Color.
    color: egui::Color32,

    /// The instance corresponding to this entry.
    instance_path: InstancePath,
}

/// Visualizer for [`GeoPoints`].
#[derive(Default)]
pub struct GeoPointsVisualizer {
    /// Objects to render.
    map_entries: Vec<GeoPointEntry>,

    /// Indices into `map_entries` corresponding to a given entity.
    entities: ahash::HashMap<EntityPathHash, Vec<usize>>,

    /// Indices into `map_entries` corresponding to specific instances.
    instances: ahash::HashMap<InstancePathHash, usize>,
}

impl IdentifiedViewSystem for GeoPointsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "GeoPoints".into()
    }
}

impl VisualizerSystem for GeoPointsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<GeoPoints>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result.query_archetype_with_history::<GeoPoints>(ctx, view_query);

            // gather all relevant chunks
            let timeline = view_query.timeline;
            let all_positions = results.iter_as(timeline, LatLon::name());
            let all_colors = results.iter_as(timeline, Color::name());
            let all_radii = results.iter_as(timeline, Radius::name());

            // fallback component values
            let fallback_color: Color =
                self.fallback_for(&ctx.query_context(data_result, &view_query.latest_at_query()));
            let fallback_radius: Radius =
                self.fallback_for(&ctx.query_context(data_result, &view_query.latest_at_query()));

            // iterate over each chunk and find all relevant component slices
            for (_index, positions, colors, radii) in re_query::range_zip_1x2(
                all_positions.component::<LatLon>(),
                all_colors.component::<Color>(),
                all_radii.component::<Radius>(),
            ) {
                // required component
                let positions = positions.as_slice();

                // optional components
                let colors = colors.as_ref().map(|c| c.as_slice()).unwrap_or(&[]);
                let radii = radii.as_ref().map(|r| r.as_slice()).unwrap_or(&[]);

                // optional components values to be used for instance clamping semantics
                let last_color = colors.last().copied().unwrap_or(fallback_color);
                let last_radii = radii.last().copied().unwrap_or(fallback_radius);

                // iterate over all instances
                for (instance_index, (position, color, radius)) in itertools::izip!(
                    positions,
                    colors.iter().chain(std::iter::repeat(&last_color)),
                    radii.iter().chain(std::iter::repeat(&last_radii)),
                )
                .enumerate()
                {
                    let entry = GeoPointEntry {
                        position: walkers::Position::from_lat_lon(
                            position.latitude(),
                            position.longitude(),
                        ),
                        //TODO(#7872): support for radius in meter
                        radius: radius.0.abs(),
                        color: color.0.into(),
                        instance_path: InstancePath::instance(
                            data_result.entity_path.clone(),
                            (instance_index as u64).into(),
                        ),
                    };

                    let next_idx = self.map_entries.len();
                    self.instances.insert(entry.instance_path.hash(), next_idx);
                    self.entities
                        .entry(data_result.entity_path.hash())
                        .or_default()
                        .push(next_idx);

                    self.map_entries.push(entry);
                }
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl GeoPointsVisualizer {
    /// Return a [`walkers::Plugin`] for this visualizer.
    pub fn plugin<'a>(
        &'a self,
        ctx: &'a ViewerContext<'a>,
        view_id: SpaceViewId,
        picked_instance: &'a mut Option<PickedInstance>,
    ) -> impl walkers::Plugin + 'a {
        GeoPointsPlugin {
            visualizer: self,
            viewer_ctx: ctx,
            view_id,
            picked_instance,
        }
    }

    /// Compute the [`super::GeoSpan`] of all the points in the visualizer.
    pub fn span(&self) -> Option<super::GeoSpan> {
        super::GeoSpan::from_lat_long(
            self.map_entries
                .iter()
                .map(|entry| (entry.position.lat(), entry.position.lon())),
        )
    }

    /// Returns a slice of entry indices matching the provided instance path.
    fn indices_for_instance(&self, instance_path: &InstancePath) -> &[usize] {
        let indices = if instance_path.instance.is_all() {
            self.entities
                .get(&instance_path.entity_path.hash())
                .map(|indices| indices.as_slice())
        } else {
            self.instances
                .get(&instance_path.hash())
                .map(|idx| std::slice::from_ref(idx))
        };

        indices.unwrap_or_default()
    }

    /// Returns entry indices corresponding to the provided item collection.
    fn indices_for_item_collection(
        &self,
        item_collection: &ItemCollection,
        view_id: SpaceViewId,
    ) -> Vec<usize> {
        item_collection
            .iter()
            .map(|(item, _)| {
                if let Item::DataResult(item_view_id, instance_path) = item {
                    if *item_view_id == view_id {
                        return self.indices_for_instance(instance_path);
                    }
                }

                // empty slice
                Default::default()
            })
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
    }
}

impl TypedComponentFallbackProvider<Color> for GeoPointsVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<Radius> for GeoPointsVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> Radius {
        Radius::from(5.0)
    }
}

re_viewer_context::impl_component_fallback_provider!(GeoPointsVisualizer => [Color, Radius]);

struct GeoPointsPlugin<'a> {
    visualizer: &'a GeoPointsVisualizer,
    viewer_ctx: &'a ViewerContext<'a>,
    view_id: SpaceViewId,
    picked_instance: &'a mut Option<PickedInstance>,
}

impl walkers::Plugin for GeoPointsPlugin<'_> {
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        response: &egui::Response,
        projector: &walkers::Projector,
    ) {
        let painter = ui.painter();

        // let's avoid computing that twice
        let projected_position = self
            .visualizer
            .map_entries
            .iter()
            .map(|entry| projector.project(entry.position).to_pos2())
            .collect::<Vec<_>>();

        //
        // First pass: draw everything without any highlight
        //

        let hover_position = response.hover_pos();
        for (entry, position) in self
            .visualizer
            .map_entries
            .iter()
            .zip(projected_position.iter())
        {
            if let Some(hover_position) = hover_position {
                let pixel_distance = hover_position.distance(*position);
                if pixel_distance < entry.radius {
                    update_picked_instance(
                        self.picked_instance,
                        Some(PickedInstance {
                            instance_path: entry.instance_path.clone(),
                            pixel_distance,
                        }),
                    );
                }
            }

            painter.circle_filled(*position, entry.radius, entry.color);
        }

        //
        // Find the indices of all entries that are part of the current selection.
        //

        let selected_entries_indices = self
            .visualizer
            .indices_for_item_collection(self.viewer_ctx.selection(), self.view_id);

        //
        // Second pass: draw highlights for everything that is selected
        //

        for index in &selected_entries_indices {
            let entry = &self.visualizer.map_entries[*index];
            let position = projected_position[*index];

            painter.circle_stroke(
                position,
                entry.radius,
                egui::Stroke::new(2.0, ui.style().visuals.selection.bg_fill),
            );
        }

        //
        // Third pass: draw the selected entries again on top of the selection highlight
        //

        for index in selected_entries_indices {
            let entry = &self.visualizer.map_entries[index];
            let position = projected_position[index];

            painter.circle_filled(position, entry.radius, entry.color);
        }

        //
        // Forth pass: draw the hovered entries
        //

        let hovered_entries_indices = self
            .visualizer
            .indices_for_item_collection(self.viewer_ctx.hovered(), self.view_id);

        for index in &hovered_entries_indices {
            let entry = &self.visualizer.map_entries[*index];
            let position = projected_position[*index];

            painter.circle_stroke(
                position,
                entry.radius,
                egui::Stroke::new(
                    2.0,
                    ui.style()
                        .visuals
                        .widgets
                        .active
                        .text_color()
                        .gamma_multiply(0.5),
                ),
            );
        }
    }
}
