use re_chunk_store::RowId;
use re_log_types::hash::Hash64;
use re_log_types::{Instance, TimeInt};
use re_renderer::renderer::GpuMeshInstance;
use re_sdk_types::Archetype as _;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::Asset3D;
use re_sdk_types::components::{AlbedoFactor, Blob};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use crate::caches::{AnyMesh, MeshCache, MeshCacheKey};
use crate::contexts::SpatialSceneVisualizerInstructionContext;

#[derive(Default)]
pub struct Asset3DVisualizer;

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
        data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        instances: &mut Vec<GpuMeshInstance>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        asset_data: impl Iterator<Item = Asset3DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for asset in asset_data {
            let primary_row_id = asset.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(Instance::ALL);

            // TODO(#5974): this is subtly wrong, the key should actually be a hash of everything that got
            // cached, which includes the media type…
            let mesh = ctx.store_ctx().memoizer(|c: &mut MeshCache| {
                let key = MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
                    query_result_hash: asset.query_result_hash,
                    media_type: asset.media_type.clone().map(Into::into),
                };

                c.entry(
                    &entity_path.to_string(),
                    key.clone(),
                    AnyMesh::Asset {
                        asset: crate::mesh_loader::NativeAsset3D {
                            bytes: &asset.blob,
                            media_type: asset.media_type.clone().map(Into::into),
                            albedo_factor: asset.albedo_factor.map(|a| a.0.into()),
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
                            // TODO(andreas): honor the culling settings from the mesh file if any.
                            cull_mode: Default::default(),
                        }
                    }));

                    data.add_bounding_box(entity_path.hash(), mesh.bbox(), world_from_pose);
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
        VisualizerQueryInfo::single_required_component::<Blob>(
            &Asset3D::descriptor_blob(),
            &Asset3D::all_components(),
        )
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView3D::identifier())
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut data = SpatialViewVisualizerData::default();
        let output = VisualizerExecutionOutput::default();
        let mut instances = Vec::new();

        use super::entity_iterator::process_archetype;
        process_archetype::<Asset3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_blobs = results.iter_required(Asset3D::descriptor_blob().component);
                if all_blobs.is_empty() {
                    return Ok(());
                }

                let all_media_types =
                    results.iter_optional(Asset3D::descriptor_media_type().component);
                let all_albedo_factors =
                    results.iter_optional(Asset3D::descriptor_albedo_factor().component);

                let query_result_hash = results.query_result_hash();

                let asset_data = re_query::range_zip_1x2(
                    all_blobs.slice::<&[u8]>(),
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

                Self::process_data(&mut data, ctx, &mut instances, spatial_ctx, asset_data);

                Ok(())
            },
        )?;

        Ok(output
            .with_draw_data([re_renderer::renderer::MeshDrawData::new(
                ctx.viewer_ctx.render_ctx(),
                &instances,
            )?
            .into()])
            .with_visualizer_data(data))
    }
}
