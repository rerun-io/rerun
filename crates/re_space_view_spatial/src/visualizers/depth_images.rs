use itertools::Itertools as _;
use nohash_hasher::IntSet;

use re_entity_db::EntityPath;
use re_log_types::{EntityPathHash, RowId, TimeInt};
use re_query::range_zip_1x3;
use re_renderer::renderer::{DepthCloud, DepthClouds};
use re_space_view::diff_component_filter;
use re_types::{
    archetypes::DepthImage,
    components::{Colormap, DepthMeter, DrawOrder, FillRatio, TensorData, ViewCoordinates},
    tensor_data::{DecodedTensor, TensorDataMeaning},
};
use re_viewer_context::{
    gpu_bridge::colormap_to_re_renderer, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    SpaceViewClass, SpaceViewSystemExecutionError, TensorDecodeCache, TensorStatsCache,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    VisualizableEntities, VisualizableFilterContext, VisualizerAdditionalApplicabilityFilter,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::{SpatialSceneEntityContext, TransformContext},
    query_pinhole_legacy,
    view_kind::SpatialSpaceViewKind,
    visualizers::{filter_visualizable_2d_entities, SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES},
    PickableImageRect, SpatialSpaceView2D, SpatialSpaceView3D,
};

use super::{
    tensor_to_textured_rect, textured_rect_utils::bounding_box_for_textured_rect,
    SpatialViewVisualizerData,
};

pub struct DepthImageVisualizer {
    pub data: SpatialViewVisualizerData,
    pub images: Vec<PickableImageRect>,
    pub depth_cloud_entities: IntSet<EntityPathHash>,
}

impl Default for DepthImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
            images: Vec::new(),
            depth_cloud_entities: IntSet::default(),
        }
    }
}

struct DepthImageComponentData<'a> {
    index: (TimeInt, RowId),

    tensor: &'a TensorData,
    colormap: Option<&'a Colormap>,
    depth_meter: Option<&'a DepthMeter>,
    fill_ratio: Option<&'a FillRatio>,
}

impl DepthImageVisualizer {
    fn process_depth_image_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        depth_clouds: &mut Vec<DepthCloud>,
        transforms: &TransformContext,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = DepthImageComponentData<'a>>,
    ) {
        let is_3d_view =
            ent_context.space_view_class_identifier == SpatialSpaceView3D::identifier();

        let entity_path = ctx.target_entity_path;
        let meaning = TensorDataMeaning::Depth;

        for data in data {
            if !data.tensor.is_shaped_like_an_image() {
                continue;
            }

            let tensor_data_row_id = data.index.1;
            let tensor = match ctx.viewer_ctx.cache.entry(|c: &mut TensorDecodeCache| {
                c.entry(tensor_data_row_id, data.tensor.0.clone())
            }) {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {entity_path}: {err}"
                    );
                    continue;
                }
            };

            let colormap = data
                .colormap
                .copied()
                .unwrap_or_else(|| self.fallback_for(ctx));

            if is_3d_view {
                if let Some(parent_pinhole_path) = transforms.parent_pinhole(entity_path) {
                    let depth_meter = data
                        .depth_meter
                        .copied()
                        .unwrap_or_else(|| self.fallback_for(ctx));
                    let fill_ratio = data.fill_ratio.copied().unwrap_or_default();

                    // NOTE: we don't pass in `world_from_obj` because this corresponds to the
                    // transform of the projection plane, which is of no use to us here.
                    // What we want are the extrinsics of the depth camera!
                    match Self::process_entity_view_as_depth_cloud(
                        ctx,
                        transforms,
                        ent_context,
                        tensor_data_row_id,
                        &tensor,
                        entity_path,
                        parent_pinhole_path,
                        colormap,
                        depth_meter,
                        fill_ratio,
                    ) {
                        Ok(cloud) => {
                            self.data.add_bounding_box(
                                entity_path.hash(),
                                cloud.world_space_bbox(),
                                glam::Affine3A::IDENTITY,
                            );
                            self.depth_cloud_entities.insert(entity_path.hash());
                            depth_clouds.push(cloud);
                            return;
                        }
                        Err(err) => {
                            re_log::warn_once!("{err}");
                        }
                    }
                };
            }

            if let Some(textured_rect) = tensor_to_textured_rect(
                ctx.viewer_ctx,
                entity_path,
                ent_context,
                tensor_data_row_id,
                &tensor,
                meaning,
                re_renderer::Rgba::WHITE,
                Some(colormap),
            ) {
                // Only update the bounding box if this is a 2D space view.
                // This is avoids a cyclic relationship where the image plane grows
                // the bounds which in turn influence the size of the image plane.
                // See: https://github.com/rerun-io/rerun/issues/3728
                if ent_context.space_view_class_identifier == SpatialSpaceView2D::identifier() {
                    self.data.add_bounding_box(
                        entity_path.hash(),
                        bounding_box_for_textured_rect(&textured_rect),
                        ent_context.world_from_entity,
                    );
                }

                self.images.push(PickableImageRect {
                    ent_path: entity_path.clone(),
                    textured_rect,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_depth_cloud(
        ctx: &QueryContext<'_>,
        transforms: &TransformContext,
        ent_context: &SpatialSceneEntityContext<'_>,
        tensor_data_row_id: RowId,
        tensor: &DecodedTensor,
        ent_path: &EntityPath,
        parent_pinhole_path: &EntityPath,
        colormap: Colormap,
        depth_meter: DepthMeter,
        radius_scale: FillRatio,
    ) -> anyhow::Result<DepthCloud> {
        re_tracing::profile_function!();

        let Some(intrinsics) =
            query_pinhole_legacy(ctx.recording(), ctx.query, parent_pinhole_path)
        else {
            anyhow::bail!("Couldn't fetch pinhole intrinsics at {parent_pinhole_path:?}");
        };

        // Place the cloud at the pinhole's location. Note that this means we ignore any 2D transforms that might be there.
        let world_from_view = transforms.reference_from_entity_ignoring_pinhole(
            parent_pinhole_path,
            ctx.recording(),
            ctx.query,
        );
        let Some(world_from_view) = world_from_view else {
            anyhow::bail!("Couldn't fetch pinhole extrinsics at {parent_pinhole_path:?}");
        };
        let world_from_rdf = world_from_view
            * glam::Affine3A::from_mat3(
                intrinsics
                    .camera_xyz
                    .unwrap_or(ViewCoordinates::RDF) // TODO(#2641): This should come from archetype
                    .from_rdf(),
            );

        let Some([height, width, _]) = tensor.image_height_width_channels() else {
            anyhow::bail!("Tensor at {ent_path:?} is not an image");
        };
        let dimensions = glam::UVec2::new(width as _, height as _);

        let debug_name = ent_path.to_string();
        let tensor_stats = ctx
            .viewer_ctx
            .cache
            .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));

        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            anyhow::bail!("No render context available for depth cloud creation");
        };

        let depth_texture = re_viewer_context::gpu_bridge::tensor_to_gpu(
            render_ctx,
            &debug_name,
            tensor_data_row_id,
            tensor,
            TensorDataMeaning::Depth,
            &tensor_stats,
            &ent_context.annotations,
            Some(colormap),
        )?;

        let world_depth_from_texture_depth = 1.0 / depth_meter.0;

        // We want point radius to be defined in a scale where the radius of a point
        // is a factor of the diameter of a pixel projected at that distance.
        let fov_y = intrinsics.fov_y().unwrap_or(1.0);
        let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * height as f32);
        let point_radius_from_world_depth = *radius_scale.0 * pixel_width_from_depth;

        Ok(DepthCloud {
            world_from_rdf,
            depth_camera_intrinsics: intrinsics.image_from_camera.0.into(),
            world_depth_from_texture_depth,
            point_radius_from_world_depth,
            max_depth_in_world: world_depth_from_texture_depth * depth_texture.range[1],
            depth_dimensions: dimensions,
            depth_texture: depth_texture.texture,
            colormap: colormap_to_re_renderer(colormap),
            outline_mask_id: ent_context.highlight.overall,
            picking_object_id: re_renderer::PickingLayerObjectId(ent_path.hash64()),
        })
    }
}

impl IdentifiedViewSystem for DepthImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "DepthImage".into()
    }
}

struct DepthImageVisualizerEntityFilter;

impl VisualizerAdditionalApplicabilityFilter for DepthImageVisualizerEntityFilter {
    fn update_applicability(&mut self, event: &re_data_store::StoreEvent) -> bool {
        diff_component_filter(event, |tensor: &re_types::components::TensorData| {
            tensor.is_shaped_like_an_image()
        })
    }
}

impl VisualizerSystem for DepthImageVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<DepthImage>()
    }

    fn applicability_filter(&self) -> Option<Box<dyn VisualizerAdditionalApplicabilityFilter>> {
        Some(Box::new(DepthImageVisualizerEntityFilter))
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_2d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut depth_clouds = Vec::new();
        let transforms = context_systems.get::<TransformContext>()?;

        super::entity_iterator::process_archetype::<Self, DepthImage, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let tensors = match results.get_required_component_dense::<TensorData>(resolver) {
                    Some(tensors) => tensors?,
                    _ => return Ok(()),
                };

                let colormap = results.get_or_empty_dense(resolver)?;
                let depth_meter = results.get_or_empty_dense(resolver)?;
                let fill_ratio = results.get_or_empty_dense(resolver)?;

                let mut data = range_zip_1x3(
                    tensors.range_indexed(),
                    colormap.range_indexed(),
                    depth_meter.range_indexed(),
                    fill_ratio.range_indexed(),
                )
                .filter_map(
                    |(&index, tensors, colormap, depth_meter, fill_ratio)| {
                        tensors.first().map(|tensor| DepthImageComponentData {
                            index,
                            tensor,
                            colormap: colormap.and_then(|colormap| colormap.first()),
                            depth_meter: depth_meter.and_then(|depth_meter| depth_meter.first()),
                            fill_ratio: fill_ratio.and_then(|fill_ratio| fill_ratio.first()),
                        })
                    },
                );

                self.process_depth_image_data(
                    ctx,
                    &mut depth_clouds,
                    transforms,
                    spatial_ctx,
                    &mut data,
                );

                Ok(())
            },
        )?;

        let mut draw_data_list = Vec::new();

        match re_renderer::renderer::DepthCloudDrawData::new(
            render_ctx,
            &DepthClouds {
                clouds: depth_clouds,
                radius_boost_in_ui_points_for_outlines: SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
            },
        ) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!(
                    "Failed to create depth cloud draw data from depth images: {err}"
                );
            }
        }
        // TODO(wumpf): Can we avoid this copy, maybe let DrawData take an iterator?
        let rectangles = self
            .images
            .iter()
            .map(|image| image.textured_rect.clone())
            .collect_vec();
        match re_renderer::renderer::RectangleDrawData::new(render_ctx, &rectangles) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!("Failed to create rectangle draw data from images: {err}");
            }
        }

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Colormap> for DepthImageVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Colormap {
        Colormap::Turbo
    }
}

impl TypedComponentFallbackProvider<DepthMeter> for DepthImageVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> DepthMeter {
        let is_integer_tensor = ctx
            .recording()
            .latest_at_component::<TensorData>(ctx.target_entity_path, ctx.query)
            .map_or(false, |tensor| tensor.dtype().is_integer());

        if is_integer_tensor { 1000.0 } else { 1.0 }.into()
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for DepthImageVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_DEPTH_IMAGE
    }
}

re_viewer_context::impl_component_fallback_provider!(DepthImageVisualizer => [Colormap, DepthMeter, DrawOrder]);
