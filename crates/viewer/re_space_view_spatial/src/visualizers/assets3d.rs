use re_chunk_store::RowId;
use re_log_types::{hash::Hash64, Instance, TimeInt};
use re_renderer::renderer::MeshInstance;
use re_renderer::RenderContext;
use re_types::{
    archetypes::Asset3D,
    components::{Blob, MediaType},
    ArrowBuffer, ArrowString, Loggable as _,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    ViewContext, ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};

use crate::{
    contexts::SpatialSceneEntityContext,
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    mesh_cache::{AnyMesh, MeshCache, MeshCacheKey},
    view_kind::SpatialSpaceViewKind,
};

pub struct Asset3DVisualizer(SpatialViewVisualizerData);

impl Default for Asset3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

struct Asset3DComponentData {
    index: (TimeInt, RowId),

    blob: ArrowBuffer<u8>,
    media_type: Option<ArrowString>,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Asset3DVisualizer {
    fn process_data(
        &mut self,
        ctx: &QueryContext<'_>,
        render_ctx: &RenderContext,
        instances: &mut Vec<MeshInstance>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Asset3DComponentData>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let mesh = Asset3D {
                blob: data.blob.clone().into(),
                media_type: data.media_type.clone().map(Into::into),
            };

            let primary_row_id = data.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(Instance::ALL);

            // TODO(#5974): this is subtly wrong, the key should actually be a hash of everything that got
            // cached, which includes the media type…
            let mesh = ctx.viewer_ctx.cache.entry(|c: &mut MeshCache| {
                c.entry(
                    &entity_path.to_string(),
                    MeshCacheKey {
                        versioned_instance_path_hash: picking_instance_hash
                            .versioned(primary_row_id),
                        query_result_hash: Hash64::ZERO,
                        media_type: data.media_type.clone().map(Into::into),
                    },
                    AnyMesh::Asset(&mesh),
                    render_ctx,
                )
            });

            if let Some(mesh) = mesh {
                re_tracing::profile_scope!("mesh instances");

                // Let's draw the mesh once for every instance transform. Because why not!
                for &world_from_pose in &ent_context.transform_info.reference_from_instances {
                    instances.extend(mesh.mesh_instances.iter().map(move |mesh_instance| {
                        let pose_from_mesh = mesh_instance.world_from_mesh;
                        let world_from_mesh = world_from_pose * pose_from_mesh;

                        MeshInstance {
                            gpu_mesh: mesh_instance.gpu_mesh.clone(),
                            world_from_mesh,
                            outline_mask_ids,
                            picking_layer_id: picking_layer_id_from_instance_path_hash(
                                picking_instance_hash,
                            ),
                            ..Default::default()
                        }
                    }));

                    self.0
                        .add_bounding_box(entity_path.hash(), mesh.bbox(), world_from_pose);
                }
            };
        }
    }
}

impl IdentifiedViewSystem for Asset3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Asset3D".into()
    }
}

impl VisualizerSystem for Asset3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Asset3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
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

        let mut instances = Vec::new();

        super::entity_iterator::process_archetype2::<Self, Asset3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt2 as _;

                let Some(all_blob_chunks) = results.get_required_chunks(&Blob::name()) else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_blobs_indexed = all_blob_chunks.iter().flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(&timeline, &Blob::name()),
                        chunk.iter_buffer::<u8>(&Blob::name())
                    )
                });
                let all_media_types = results.iter_as(timeline, MediaType::name());

                let data = re_query2::range_zip_1x1(all_blobs_indexed, all_media_types.string())
                    .filter_map(|(index, blobs, media_types)| {
                        blobs.first().map(|blob| Asset3DComponentData {
                            index,
                            blob: blob.clone(),
                            media_type: media_types
                                .and_then(|media_types| media_types.first().cloned()),
                        })
                    });

                self.process_data(ctx, render_ctx, &mut instances, spatial_ctx, data);

                Ok(())
            },
        )?;

        match re_renderer::renderer::MeshDrawData::new(render_ctx, &instances) {
            Ok(draw_data) => Ok(vec![draw_data.into()]),
            Err(err) => {
                re_log::error_once!("Failed to create mesh draw data from mesh instances: {err}");
                Ok(Vec::new()) // TODO(andreas): Pass error on?
            }
        }
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(Asset3DVisualizer => []);
