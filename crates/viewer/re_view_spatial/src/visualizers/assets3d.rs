use arrow::buffer::ScalarBuffer;

use re_chunk_store::RowId;
use re_log_types::{hash::Hash64, Instance, TimeInt};
use re_renderer::renderer::GpuMeshInstance;
use re_types::{
    archetypes::Asset3D,
    components::{AlbedoFactor, Blob, MediaType},
    ArrowString, Component as _,
};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, QueryContext, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};

use crate::{
    contexts::SpatialSceneEntityContext,
    mesh_cache::{AnyMesh, MeshCache, MeshCacheKey},
    view_kind::SpatialViewKind,
};

pub struct Asset3DVisualizer(SpatialViewVisualizerData);

impl Default for Asset3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

struct Asset3DComponentData<'a> {
    index: (TimeInt, RowId),
    query_result_hash: Hash64,

    blob: ScalarBuffer<u8>,
    media_type: Option<ArrowString>,
    albedo_factor: Option<&'a AlbedoFactor>,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Asset3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        instances: &mut Vec<GpuMeshInstance>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Asset3DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let primary_row_id = data.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(Instance::ALL);

            // TODO(#5974): this is subtly wrong, the key should actually be a hash of everything that got
            // cached, which includes the media type…
            let mesh = ctx
                .viewer_ctx
                .store_context
                .caches
                .entry(|c: &mut MeshCache| {
                    let key = MeshCacheKey {
                        versioned_instance_path_hash: picking_instance_hash
                            .versioned(primary_row_id),
                        query_result_hash: data.query_result_hash,
                        media_type: data.media_type.clone().map(Into::into),
                    };

                    c.entry(
                        &entity_path.to_string(),
                        key.clone(),
                        AnyMesh::Asset {
                            asset: crate::mesh_loader::NativeAsset3D {
                                bytes: &data.blob,
                                media_type: data.media_type.clone().map(Into::into),
                                albedo_factor: data.albedo_factor.map(|a| a.0.into()),
                            },
                        },
                        ctx.viewer_ctx.render_ctx(),
                    )
                });

            if let Some(mesh) = mesh {
                re_tracing::profile_scope!("mesh instances");

                // Let's draw the mesh once for every instance transform.
                // TODO(#7026): This a rare form of hybrid joining.
                for &world_from_pose in &ent_context.transform_info.reference_from_instances {
                    instances.extend(mesh.mesh_instances.iter().map(move |mesh_instance| {
                        let pose_from_mesh = mesh_instance.world_from_mesh;
                        let world_from_mesh = world_from_pose * pose_from_mesh;

                        GpuMeshInstance {
                            gpu_mesh: mesh_instance.gpu_mesh.clone(),
                            world_from_mesh,
                            outline_mask_ids,
                            picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                                picking_instance_hash,
                            ),
                            additive_tint: re_renderer::Color32::TRANSPARENT,
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
        entities: MaybeVisualizableEntities,
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let mut instances = Vec::new();

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, Asset3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_blob_chunks) = results.get_required_chunks(&Blob::name()) else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_blobs_indexed =
                    iter_slices::<&[u8]>(&all_blob_chunks, timeline, Blob::name());
                let all_media_types = results.iter_as(timeline, MediaType::name());
                let all_albedo_factors = results.iter_as(timeline, AlbedoFactor::name());

                let query_result_hash = results.query_result_hash();

                let data = re_query::range_zip_1x2(
                    all_blobs_indexed,
                    all_media_types.slice::<String>(),
                    all_albedo_factors.slice::<u32>(),
                )
                .filter_map(|(index, blobs, media_types, albedo_factors)| {
                    blobs.first().map(|blob| Asset3DComponentData {
                        index,
                        query_result_hash,
                        blob: blob.clone(),
                        media_type: media_types
                            .and_then(|media_types| media_types.first().cloned()),
                        albedo_factor: albedo_factors
                            .map_or(&[] as &[AlbedoFactor], |albedo_factors| {
                                bytemuck::cast_slice(albedo_factors)
                            })
                            .first(),
                    })
                });

                self.process_data(ctx, &mut instances, spatial_ctx, data);

                Ok(())
            },
        )?;

        match re_renderer::renderer::MeshDrawData::new(ctx.viewer_ctx.render_ctx(), &instances) {
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

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(Asset3DVisualizer => []);
