use std::sync::Arc;

use itertools::Itertools as _;
use re_log_types::Instance;
use re_renderer::renderer::{GpuMeshInstance, LineStripFlags};
use re_renderer::{PickingLayerInstanceId, mesh};
use re_sdk_types::archetypes::Triangles3D;
use re_sdk_types::components::{ClassId, Color, FillMode, Position3D, Radius, ShowLabels};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{Archetype as _, ArrowString};
use re_view::{process_annotation_slices, process_color_slice};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::utilities::LabeledBatch;
use super::{SpatialViewVisualizerData, process_labels_3d, process_radius_slice};
use crate::contexts::SpatialSceneVisualizerInstructionContext;

// ---

#[derive(Default)]
pub struct Triangles3DVisualizer;

impl Triangles3DVisualizer {
    fn process_data<'a>(
        data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        view_query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        mesh_instances: &mut Vec<GpuMeshInstance>,
        results_iter: impl Iterator<Item = Triangles3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;

        for ent_data in results_iter {
            let num_triangles = ent_data.vertex_positions.len() / 3;
            if num_triangles == 0 {
                continue;
            }

            let annotation_infos = process_annotation_slices(
                view_query.latest_at,
                num_triangles,
                ent_data.class_ids,
                &ent_context.annotations,
            );
            let colors = process_color_slice(
                ctx,
                Triangles3D::descriptor_colors().component,
                num_triangles,
                &annotation_infos,
                ent_data.colors,
            );
            let radii = process_radius_slice(
                ctx,
                entity_path,
                num_triangles,
                ent_data.line_radii,
                Triangles3D::descriptor_line_radii().component,
            );

            let world_from_obj = ent_context
                .transform_info
                .single_transform_required_for_entity(entity_path, Triangles3D::name())
                .as_affine3a();

            let mut obj_space_bounding_box = macaw::BoundingBox::nothing();
            let mut centroids = Vec::with_capacity(num_triangles);

            if ent_data.fill_mode.has_wireframe() {
                let mut line_batch = line_builder
                    .batch(entity_path.to_string())
                    .depth_offset(ent_context.depth_offset)
                    .world_from_obj(world_from_obj)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

                for (i, (triangle, radius, &color)) in ent_data
                    .vertex_positions
                    .chunks_exact(3)
                    .zip(radii.iter())
                    .zip(&colors)
                    .map(|((triangle, radius), color)| (triangle, radius, color))
                    .enumerate()
                {
                    let points = [
                        glam::Vec3::from(triangle[0].0),
                        glam::Vec3::from(triangle[1].0),
                        glam::Vec3::from(triangle[2].0),
                        glam::Vec3::from(triangle[0].0),
                    ];

                    let lines = line_batch
                        .add_strip(points.into_iter())
                        .flags(LineStripFlags::STRIP_FLAGS_OUTWARD_EXTENDING_ROUND_CAPS)
                        .color(color)
                        .radius(*radius)
                        .picking_instance_id(PickingLayerInstanceId(i as _));

                    if let Some(outline_mask_ids) = ent_context
                        .highlight
                        .instances
                        .get(&Instance::from(i as u64))
                    {
                        lines.outline_mask_ids(*outline_mask_ids);
                    }
                }
            }

            let vertex_positions: Vec<_> = ent_data
                .vertex_positions
                .iter()
                .take(num_triangles * 3)
                .map(|position| {
                    let position = glam::Vec3::from(position.0);
                    obj_space_bounding_box.extend(position);
                    position
                })
                .collect();

            for triangle in vertex_positions.chunks_exact(3) {
                centroids.push((triangle[0] + triangle[1] + triangle[2]) / 3.0);
            }

            if ent_data.fill_mode.has_solid() {
                let triangle_indices: Vec<_> = (0..vertex_positions.len() as u32)
                    .tuples::<(_, _, _)>()
                    .map(glam::UVec3::from)
                    .collect();

                let vertex_normals: Vec<_> = vertex_positions
                    .chunks_exact(3)
                    .flat_map(|triangle| {
                        let normal = (triangle[1] - triangle[0])
                            .cross(triangle[2] - triangle[0])
                            .normalize_or_zero();
                        [normal, normal, normal]
                    })
                    .collect();

                let vertex_colors: Vec<_> = colors
                    .iter()
                    .flat_map(|color| {
                        let color =
                            re_renderer::Rgba32Unmul::from_rgba_unmul_array(color.to_array());
                        [color, color, color]
                    })
                    .take(vertex_positions.len())
                    .collect();

                let albedo_factor = if ent_data.fill_mode == FillMode::TransparentFillMajorWireframe
                {
                    re_sdk_types::datatypes::Rgba32::from_unmultiplied_rgba(255, 255, 255, 64)
                } else {
                    re_sdk_types::datatypes::Rgba32::WHITE
                };

                let num_indices = triangle_indices.len() * 3;
                let mesh = mesh::CpuMesh {
                    label: entity_path.to_string().into(),
                    triangle_indices,
                    vertex_positions,
                    vertex_normals,
                    vertex_colors,
                    vertex_texcoords: vec![glam::Vec2::ZERO; num_triangles * 3],
                    materials: smallvec::smallvec![mesh::Material {
                        label: entity_path.to_string().into(),
                        index_range: 0..num_indices as _,
                        albedo: ctx
                            .render_ctx()
                            .texture_manager_2d
                            .white_texture_unorm_handle()
                            .clone(),
                        albedo_factor: albedo_factor.into(),
                    }],
                    bbox: obj_space_bounding_box,
                };

                let gpu_mesh =
                    re_renderer::mesh::GpuMesh::new(ctx.render_ctx(), &mesh).map_err(|err| {
                        ViewSystemExecutionError::DrawDataCreationError(Arc::new(err))
                    })?;

                mesh_instances.push(GpuMeshInstance {
                    gpu_mesh: Arc::new(gpu_mesh),
                    world_from_mesh: world_from_obj,
                    outline_mask_ids: ent_context.highlight.index_outline_mask(Instance::ALL),
                    picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                        re_entity_db::InstancePathHash::entity_all(entity_path),
                    ),
                    additive_tint: re_renderer::Color32::BLACK,
                    cull_mode: None,
                });
            }

            data.add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);
            data.ui_labels.extend(process_labels_3d(
                LabeledBatch {
                    entity_path,
                    visualizer_instruction: ent_context.visualizer_instruction,
                    num_instances: num_triangles,
                    overall_position: obj_space_bounding_box.center(),
                    instance_positions: centroids.into_iter(),
                    labels: &ent_data.labels,
                    colors: &colors,
                    show_labels: ent_data.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(ctx, Triangles3D::descriptor_show_labels().component)
                    }),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }

        Ok(())
    }
}

struct Triangles3DComponentData<'a> {
    vertex_positions: &'a [Position3D],
    colors: &'a [Color],
    line_radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Triangles3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Triangles3D"
        )
    }
}

impl VisualizerSystem for Triangles3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Position3D>(
            &Triangles3D::descriptor_vertex_positions(),
            &Triangles3D::all_components(),
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
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );
        let mut mesh_instances = Vec::new();

        use super::entity_iterator::process_archetype;
        process_archetype::<Triangles3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_vertex_positions =
                    results.iter_required(Triangles3D::descriptor_vertex_positions().component);
                let all_colors = results.iter_optional(Triangles3D::descriptor_colors().component);
                let all_line_radii =
                    results.iter_optional(Triangles3D::descriptor_line_radii().component);
                let all_fill_modes =
                    results.iter_optional(Triangles3D::descriptor_fill_mode().component);
                let all_labels = results.iter_optional(Triangles3D::descriptor_labels().component);
                let all_show_labels =
                    results.iter_optional(Triangles3D::descriptor_show_labels().component);
                let all_class_ids =
                    results.iter_optional(Triangles3D::descriptor_class_ids().component);

                let results_iter = re_query::range_zip_1x6(
                    all_vertex_positions.slice::<[f32; 3]>(),
                    all_colors.slice::<u32>(),
                    all_line_radii.slice::<f32>(),
                    all_fill_modes.slice::<u8>(),
                    all_labels.slice::<String>(),
                    all_show_labels.slice::<bool>(),
                    all_class_ids.slice::<u16>(),
                )
                .map(
                    |(
                        _index,
                        vertex_positions,
                        colors,
                        line_radii,
                        fill_modes,
                        labels,
                        show_labels,
                        class_ids,
                    )| Triangles3DComponentData {
                        vertex_positions: bytemuck::cast_slice(vertex_positions),
                        colors: colors.map_or(&[], bytemuck::cast_slice),
                        line_radii: line_radii.map_or(&[], bytemuck::cast_slice),
                        fill_mode: fill_modes
                            .and_then(|s| FillMode::from_integer_slice(s).next()?)
                            .unwrap_or_default(),
                        labels: labels.unwrap_or_default(),
                        show_labels: show_labels
                            .map(|b| !b.is_empty() && b.value(0))
                            .map(Into::into),
                        class_ids: class_ids.map_or(&[], bytemuck::cast_slice),
                    },
                );

                Self::process_data(
                    &mut data,
                    ctx,
                    view_query,
                    spatial_ctx,
                    &mut line_builder,
                    &mut mesh_instances,
                    results_iter,
                )
            },
        )?;

        Ok(output
            .with_draw_data([
                re_renderer::renderer::MeshDrawData::new(
                    ctx.viewer_ctx.render_ctx(),
                    &mesh_instances,
                )?
                .into(),
                line_builder.into_draw_data()?.into(),
            ])
            .with_visualizer_data(data))
    }
}
