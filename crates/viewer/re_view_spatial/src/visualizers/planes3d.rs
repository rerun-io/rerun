use std::iter;

use re_sdk_types::archetypes::Planes3D;
use re_sdk_types::components::{
    ClassId, Color, FillMode, HalfSize2D, HalfSize3D, Plane3D, Radius, ShowLabels,
};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{Archetype as _, ArrowString, components};
use re_view::clamped_or_else;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use super::utilities::{ProcMeshBatch, ProcMeshDrawableBuilder};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::proc_mesh;

// ---
#[derive(Default)]
pub struct Planes3DVisualizer;

impl Planes3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        batches: impl Iterator<Item = Planes3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        for batch in batches {
            let num_instances = batch.planes.len().max(batch.half_sizes.len());
            let planes = clamped_or_else(batch.planes, || Plane3D::XY).take(num_instances);
            let half_sizes =
                clamped_or_else(batch.half_sizes, HalfSize2D::default).take(num_instances);

            let mut centers = Vec::with_capacity(num_instances);
            let mut quaternions = Vec::with_capacity(num_instances);
            let mut half_sizes3d = Vec::with_capacity(num_instances);

            for (plane, half_size) in std::iter::zip(planes, half_sizes) {
                let (center, quaternion) = plane_center_and_rotation(plane);
                centers.push(components::Translation3D::from(center));
                quaternions.push(components::RotationQuat::from(quaternion));
                half_sizes3d.push(HalfSize3D::new(half_size.x(), half_size.y(), 0.0));
            }

            builder.add_batch(
                query_context,
                ent_context,
                Planes3D::descriptor_colors().component,
                Planes3D::descriptor_line_radii().component,
                Planes3D::descriptor_show_labels().component,
                glam::Affine3A::IDENTITY,
                ProcMeshBatch {
                    half_sizes: &half_sizes3d,
                    centers: &centers,
                    rotation_axis_angles: &[],
                    quaternions: &quaternions,
                    meshes: iter::repeat(proc_mesh::ProcMeshKey::Plane),
                    fill_modes: iter::repeat(batch.fill_mode),
                    line_radii: batch.line_radii,
                    colors: batch.colors,
                    labels: &batch.labels,
                    show_labels: batch.show_labels,
                    class_ids: batch.class_ids,
                },
            )?;
        }

        Ok(())
    }
}

// ---

struct Planes3DComponentData<'a> {
    planes: &'a [Plane3D],
    half_sizes: &'a [HalfSize2D],
    colors: &'a [Color],
    labels: Vec<ArrowString>,
    line_radii: &'a [Radius],
    class_ids: &'a [ClassId],
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Planes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Planes3D"
        )
    }
}

impl VisualizerSystem for Planes3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Plane3D>(
            &Planes3D::descriptor_planes(),
            &Planes3D::all_components(),
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
        let output = VisualizerExecutionOutput::default();
        let mut data = SpatialViewVisualizerData::default();
        let mut builder = ProcMeshDrawableBuilder::new(
            &mut data,
            ctx.viewer_ctx.render_ctx(),
            view_query,
            &output,
            "planes3d",
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Planes3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_planes = results.iter_required(Planes3D::descriptor_planes().component);
                let all_half_sizes =
                    results.iter_optional(Planes3D::descriptor_half_sizes().component);
                let all_colors = results.iter_optional(Planes3D::descriptor_colors().component);
                let all_labels = results.iter_optional(Planes3D::descriptor_labels().component);
                let all_fill_modes =
                    results.iter_optional(Planes3D::descriptor_fill_mode().component);
                let all_line_radii =
                    results.iter_optional(Planes3D::descriptor_line_radii().component);
                let all_show_labels =
                    results.iter_optional(Planes3D::descriptor_show_labels().component);
                let all_class_ids =
                    results.iter_optional(Planes3D::descriptor_class_ids().component);

                let data = re_query::range_zip_1x7(
                    all_planes.slice::<[f32; 4]>(),
                    all_half_sizes.slice::<[f32; 2]>(),
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
                        planes,
                        half_sizes,
                        colors,
                        line_radii,
                        fill_modes,
                        labels,
                        show_labels,
                        class_ids,
                    )| Planes3DComponentData {
                        planes: bytemuck::cast_slice(planes),
                        half_sizes: half_sizes.map_or(&[], bytemuck::cast_slice),
                        colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                        labels: labels.unwrap_or_default(),
                        line_radii: line_radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                        fill_mode: fill_modes
                            .and_then(|s| FillMode::from_integer_slice(s).next()?)
                            .unwrap_or_default(),
                        class_ids: class_ids
                            .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                        show_labels: show_labels
                            .map(|b| !b.is_empty() && b.value(0))
                            .map(Into::into),
                    },
                );

                Self::process_data(&mut builder, ctx, spatial_ctx, data)?;

                Ok(())
            },
        )?;

        let draw_data = builder.into_draw_data()?;
        Ok(output.with_draw_data(draw_data).with_visualizer_data(data))
    }
}

fn plane_center_and_rotation(plane: Plane3D) -> (glam::Vec3, glam::Quat) {
    let raw = plane.0.0;
    let normal = glam::Vec3::new(raw[0], raw[1], raw[2]);
    let length = normal.length();
    let (normal, distance) = if normal.is_finite() && length > 0.0 {
        (normal / length, raw[3] / length)
    } else {
        (glam::Vec3::Z, 0.0)
    };

    (
        normal * distance,
        glam::Quat::from_rotation_arc(glam::Vec3::Z, normal),
    )
}
