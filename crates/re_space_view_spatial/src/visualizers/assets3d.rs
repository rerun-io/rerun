use re_entity_db::EntityPath;
use re_log_types::{RowId, TimeInt};
use re_query2::{range_zip_1x2, Results};
use re_renderer::renderer::MeshInstance;
use re_types::{
    archetypes::Asset3D,
    components::{Blob, InstanceKey, MediaType, OutOfTreeTransform3D},
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};
use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
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
    asset: Asset3D,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Asset3DVisualizer {
    fn process_data(
        &mut self,
        ctx: &ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Asset3DComponentData>,
    ) {
        for data in data {
            let entity_from_pose = data.asset.transform.clone();
            let media_type = data.asset.media_type.clone();

            let mesh = Asset3D {
                // NOTE: Don't even try to cache the transform!
                transform: None,

                ..data.asset.clone()
            };

            let primary_row_id = data.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_splat(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(InstanceKey::SPLAT);

            // TODO(#3232): this is subtly wrong, the key should actually be a hash of everything that got
            // cached, which includes the media type…
            let mesh = ctx.cache.entry(|c: &mut MeshCache| {
                c.entry(
                    &entity_path.to_string(),
                    MeshCacheKey {
                        versioned_instance_path_hash: picking_instance_hash
                            .versioned(primary_row_id),
                        media_type,
                    },
                    AnyMesh::Asset(&mesh),
                    ctx.render_ctx,
                )
            });

            if let Some(mesh) = mesh {
                re_tracing::profile_scope!("mesh instances");

                let world_from_pose = ent_context.world_from_entity
                    * entity_from_pose.map_or(glam::Affine3A::IDENTITY, |t| t.0.into());

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
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut instances = Vec::new();

        // TODO(#5974): this should use the cached APIs, but for that we first need to figure out
        // how to distribute memory budgets across all caching layers, and how to GC them.
        super::entity_iterator::process_archetype_uncached::<Asset3DVisualizer, Asset3D, _>(
            ctx,
            view_query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, entity_path, _entity_props, spatial_ctx, results| match results {
                Results::LatestAt(_query, results) => {
                    re_tracing::profile_scope!(format!("{entity_path} @ {_query:?}"));

                    use crate::visualizers::LatestAtResultsExt as _;

                    let resolver = ctx.recording().resolver();

                    let blob = match results
                        .get_dense::<Blob>(resolver)
                        .map(|res| res.map(|mut blobs| blobs.pop()))
                    {
                        Some(Ok(Some(blob))) => blob,
                        Some(Err(err)) => return Err(err.into()),
                        _ => return Ok(()),
                    };

                    let data = {
                        Asset3DComponentData {
                            index: results.compound_index,
                            asset: Asset3D {
                                blob,
                                media_type: results
                                    .get_dense::<MediaType>(resolver)
                                    .map_or(Ok(None), |v| v.map(|mut v| v.pop()))?,
                                transform: results
                                    .get_dense::<OutOfTreeTransform3D>(resolver)
                                    .map_or(Ok(None), |v| v.map(|mut v| v.pop()))?,
                            },
                        }
                    };

                    self.process_data(
                        ctx,
                        &mut instances,
                        entity_path,
                        spatial_ctx,
                        std::iter::once(data),
                    );
                    Ok(())
                }

                Results::Range(_query, results) => {
                    re_tracing::profile_scope!(format!("{entity_path} @ {_query:?}"));

                    use crate::visualizers::RangeResultsExt as _;

                    let resolver = ctx.recording().resolver();

                    let Some(blobs) = results.get_dense::<Blob>(resolver) else {
                        return Ok(());
                    };

                    let media_types = results.get_or_empty_dense(resolver);
                    let transforms = results.get_or_empty_dense(resolver);

                    let data = range_zip_1x2(blobs, media_types, transforms).filter_map(
                        |(index, mut blob, media_type, transform)| {
                            blob.pop().map(|blob| Asset3DComponentData {
                                index,
                                asset: Asset3D {
                                    blob,
                                    media_type: media_type.and_then(|mut props| props.pop()),
                                    transform: transform.and_then(|mut mats| mats.pop()),
                                },
                            })
                        },
                    );

                    self.process_data(ctx, &mut instances, entity_path, spatial_ctx, data);
                    Ok(())
                }
            },
        )?;

        match re_renderer::renderer::MeshDrawData::new(ctx.render_ctx, &instances) {
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
}
