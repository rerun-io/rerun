use itertools::Itertools;
use re_data_store::EntityPath;
use re_query::{ArchetypeView, QueryError};
use re_renderer::renderer::MeshInstance;
use re_types::{
    archetypes::Asset3D,
    components::{Blob, InstanceKey, MediaType, OutOfTreeTransform3D},
    datatypes::Transform3D,
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
    ViewQuery, ViewerContext,
};

use super::{entity_iterator::process_archetype_views, SpatialViewPartData};
use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    mesh_cache::{AnyMesh, MeshCache},
    view_kind::SpatialSpaceViewKind,
};

pub struct Asset3DPart(SpatialViewPartData);

impl Default for Asset3DPart {
    fn default() -> Self {
        Self(SpatialViewPartData::new(Some(SpatialSpaceViewKind::ThreeD)))
    }
}

impl Asset3DPart {
    fn process_arch_view(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        arch_view: &ArchetypeView<Asset3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let transform = arch_view
            .iter_raw_optional_component::<OutOfTreeTransform3D>()?
            .and_then(|mut batch| batch.next());

        let mesh = Asset3D {
            data: arch_view
                .iter_required_component::<Blob>()?
                .next()
                .ok_or_else(|| re_types::DeserializationError::MissingData {
                    backtrace: re_types::_Backtrace::new_unresolved(),
                })?,
            media_type: arch_view
                .iter_raw_optional_component::<MediaType>()?
                .and_then(|mut batch| batch.next()),
            // NOTE: Don't even try to cache the transform!
            transform: None,
        };

        let primary_row_id = arch_view.primary_row_id();
        let picking_instance_hash = re_data_store::InstancePathHash::entity_splat(ent_path);
        let outline_mask_ids = ent_context.highlight.index_outline_mask(InstanceKey::SPLAT);

        // TODO(#3232): this is subtly wrong, the key should actually be a hash of everything that got
        // cached, which includes the media type...
        let mesh = ctx.cache.entry(|c: &mut MeshCache| {
            c.entry(
                &ent_path.to_string(),
                picking_instance_hash.versioned(primary_row_id),
                AnyMesh::Asset(&mesh),
                ctx.render_ctx,
            )
        });

        if let Some(mesh) = mesh {
            let mut mesh_instances = mesh
                .mesh_instances
                .iter()
                .map(move |mesh_instance| MeshInstance {
                    gpu_mesh: mesh_instance.gpu_mesh.clone(),
                    world_from_mesh: ent_context.world_from_obj * mesh_instance.world_from_mesh,
                    outline_mask_ids,
                    picking_layer_id: picking_layer_id_from_instance_path_hash(
                        picking_instance_hash,
                    ),
                    ..Default::default()
                })
                .collect_vec();

            // Apply the out-of-tree transform.
            if let Some(transform) = transform.as_ref() {
                let (scale, rotation, translation) =
                    glam::Affine3A::from(transform).to_scale_rotation_translation();
                let transform =
                    glam::Affine3A::from_scale_rotation_translation(scale, rotation, translation);
                for instance in &mut mesh_instances {
                    instance.world_from_mesh = transform * instance.world_from_mesh;
                }
            }

            let bbox = re_renderer::importer::calculate_bounding_box(&mesh_instances);

            instances.extend(
                mesh_instances
                    .iter()
                    .map(move |mesh_instance| MeshInstance {
                        gpu_mesh: mesh_instance.gpu_mesh.clone(),
                        world_from_mesh: ent_context.world_from_obj * mesh_instance.world_from_mesh,
                        outline_mask_ids,
                        picking_layer_id: picking_layer_id_from_instance_path_hash(
                            picking_instance_hash,
                        ),
                        ..Default::default()
                    }),
            );

            self.0.extend_bounding_box(bbox, ent_context.world_from_obj);
        };

        Ok(())
    }
}

impl NamedViewSystem for Asset3DPart {
    fn name() -> re_viewer_context::ViewSystemName {
        "Asset3D".into()
    }
}

impl ViewPartSystem for Asset3DPart {
    fn required_components(&self) -> ComponentNameSet {
        Asset3D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Asset3D::indicator_component()).collect()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut instances = Vec::new();

        process_archetype_views::<Asset3DPart, Asset3D, { Asset3D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, ent_path, arch_view, ent_context| {
                self.process_arch_view(ctx, &mut instances, &arch_view, ent_path, ent_context)
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
