use re_chunk_store::RowId;
use re_log_types::hash::Hash64;
use re_log_types::{Instance, TimeInt};
use re_renderer::renderer::GpuMeshInstance;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::Asset3D;
use re_sdk_types::components::AlbedoFactor;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use crate::caches::{AnyMesh, MeshCache, MeshCacheKey};
use crate::contexts::SpatialSceneEntityContext;
use crate::view_kind::SpatialViewKind;

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

    blob: re_sdk_types::datatypes::Blob,
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
            let mesh = ctx.store_ctx().caches.entry(|c: &mut MeshCache| {
                let key = MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
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
                    ctx.render_ctx(),
                )
            });

            if let Some(mesh) = mesh {
                re_tracing::profile_scope!("mesh instances");

                // Let's draw the mesh once for every instance transform.
                // TODO(#7026): This a rare form of hybrid joining.
                for &world_from_pose in ent_context.transform_info.target_from_instances() {
                    let world_from_pose = world_from_pose.as_affine3a();
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
                            additive_tint: re_renderer::Color32::BLACK,
                        }
                    }));

                    self.0
                        .add_bounding_box(entity_path.hash(), mesh.bbox(), world_from_pose);
                }
            }
        }
    }
}

impl IdentifiedViewSystem for Asset3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Asset3D".into()
    }
}

impl VisualizerSystem for Asset3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Asset3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();
        let preferred_view_kind = self.0.preferred_view_kind;
        let mut instances = Vec::new();

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, Asset3D, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let all_blob_chunks =
                    results.get_required_chunk(Asset3D::descriptor_blob().component);
                if all_blob_chunks.is_empty() {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_blobs_indexed = iter_slices::<&[u8]>(&all_blob_chunks, timeline);
                let all_media_types =
                    results.iter_as(timeline, Asset3D::descriptor_media_type().component);
                let all_albedo_factors =
                    results.iter_as(timeline, Asset3D::descriptor_albedo_factor().component);

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
                        blob: blob.clone().into(),
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

        Ok(
            output.with_draw_data([re_renderer::renderer::MeshDrawData::new(
                ctx.viewer_ctx.render_ctx(),
                &instances,
            )?
            .into()]),
        )
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }
}
