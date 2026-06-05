use std::sync::Arc;
use std::time::{Duration, Instant};

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use re_renderer::device_caps;
use re_renderer::mesh::{CpuMesh, GpuMesh, Material};
use re_renderer::renderer::{
    GpuMeshInstance, MeshDrawData, VoxelGridDrawData, VoxelGridInstance, VoxelGridOptions,
};
use re_renderer::{
    Color32, OutlineMaskPreference, PickingLayerId, PickingLayerInstanceId, PickingLayerObjectId,
    RenderConfig, RenderContext, Rgba32Unmul, UnalignedColor32,
};
use smallvec::smallvec;

fn render_context() -> RenderContext {
    let instance = wgpu::Instance::new(device_caps::testing_instance_descriptor());
    let adapter = pollster::block_on(device_caps::select_testing_adapter(&instance));
    let device_caps =
        device_caps::DeviceCaps::from_adapter(&adapter).expect("Failed to determine device caps");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&device_caps.device_descriptor()))
            .expect("Failed to request device");

    RenderContext::new(
        &adapter,
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        |_| RenderConfig::testing(),
    )
    .expect("Failed to create render context")
}

fn voxel_index(i: usize) -> glam::IVec3 {
    glam::IVec3::new(
        (i % 100) as i32,
        ((i / 100) % 100) as i32,
        (i / 10_000) as i32,
    )
}

#[derive(Clone, Copy)]
struct SourceVoxel {
    index: glam::IVec3,
    color: Color32,
    picking_instance_id: PickingLayerInstanceId,
}

fn source_voxels(len: usize) -> Vec<SourceVoxel> {
    (0..len)
        .map(|i| SourceVoxel {
            index: voxel_index(i),
            color: UnalignedColor32([
                ((i * 53) & 0xFF) as u8,
                ((i * 97) & 0xFF) as u8,
                ((i * 193) & 0xFF) as u8,
                255,
            ])
            .into(),
            picking_instance_id: PickingLayerInstanceId(i as _),
        })
        .collect()
}

fn voxel_instances(
    source: &[SourceVoxel],
    voxel_size: glam::Vec3,
) -> (Vec<VoxelGridInstance>, glam::Vec3A) {
    let mut bbox = macaw::BoundingBox::nothing();
    let instances = source
        .iter()
        .map(|voxel| {
            let min = voxel.index.as_vec3() * voxel_size;
            let max = (voxel.index + glam::IVec3::ONE).as_vec3() * voxel_size;
            bbox = bbox.union(macaw::BoundingBox::from_min_max(min, max));

            VoxelGridInstance {
                index: voxel.index,
                color: voxel.color,
                picking_instance_id: voxel.picking_instance_id,
            }
        })
        .collect();

    (instances, bbox.center().into())
}

fn cube_gpu_mesh(ctx: &RenderContext) -> Arc<GpuMesh> {
    let vertex_positions = vec![
        glam::Vec3::new(-0.5, -0.5, -0.5),
        glam::Vec3::new(0.5, -0.5, -0.5),
        glam::Vec3::new(0.5, 0.5, -0.5),
        glam::Vec3::new(-0.5, 0.5, -0.5),
        glam::Vec3::new(-0.5, -0.5, 0.5),
        glam::Vec3::new(0.5, -0.5, 0.5),
        glam::Vec3::new(0.5, 0.5, 0.5),
        glam::Vec3::new(-0.5, 0.5, 0.5),
    ];
    let triangle_indices = vec![
        glam::UVec3::new(0, 2, 1),
        glam::UVec3::new(0, 3, 2),
        glam::UVec3::new(4, 5, 6),
        glam::UVec3::new(4, 6, 7),
        glam::UVec3::new(0, 1, 5),
        glam::UVec3::new(0, 5, 4),
        glam::UVec3::new(2, 3, 7),
        glam::UVec3::new(2, 7, 6),
        glam::UVec3::new(1, 2, 6),
        glam::UVec3::new(1, 6, 5),
        glam::UVec3::new(3, 0, 4),
        glam::UVec3::new(3, 4, 7),
    ];
    let bbox = re_renderer::util::bounding_box_from_points(vertex_positions.iter().copied());

    Arc::new(
        GpuMesh::new(
            ctx,
            &CpuMesh {
                label: "benchmark_cube".into(),
                triangle_indices,
                vertex_colors: vec![Rgba32Unmul::WHITE; vertex_positions.len()],
                vertex_normals: vec![glam::Vec3::ZERO; vertex_positions.len()],
                vertex_texcoords: vec![glam::Vec2::ZERO; vertex_positions.len()],
                vertex_positions,
                materials: smallvec![Material {
                    label: "opaque_material".into(),
                    index_range: 0..36,
                    albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
                    albedo_factor: re_renderer::Rgba::WHITE,
                }],
                bbox,
            },
        )
        .expect("Failed to create benchmark cube mesh"),
    )
}

fn mesh_instances(
    source: &[SourceVoxel],
    cube_mesh: &Arc<GpuMesh>,
) -> (Vec<GpuMeshInstance>, macaw::BoundingBox) {
    let mut bbox = macaw::BoundingBox::nothing();
    let instances = source
        .iter()
        .map(|voxel| {
            let center = voxel.index.as_vec3() + glam::Vec3::splat(0.5);
            let world_from_mesh = glam::Affine3A::from_translation(center);
            bbox = bbox.union(cube_mesh.bbox.transform_affine3(&world_from_mesh));

            GpuMeshInstance {
                gpu_mesh: Arc::clone(cube_mesh),
                world_from_mesh,
                additive_tint: UnalignedColor32([
                    voxel.color.r(),
                    voxel.color.g(),
                    voxel.color.b(),
                    voxel.color.a() / 4,
                ])
                .into(),
                outline_mask_ids: OutlineMaskPreference::NONE,
                picking_layer_id: PickingLayerId::default(),
                cull_mode: None,
            }
        })
        .collect();

    (instances, bbox)
}

fn finish_frame(ctx: &mut RenderContext) {
    ctx.before_submit();
    ctx.begin_frame();
}

fn bench_voxel_grid_draw_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("voxel_grid_draw_data");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(2));

    for len in [10_000, 100_000] {
        let source = source_voxels(len);
        let voxel_size = glam::Vec3::ONE;
        let mut voxel_ctx = render_context();
        let (voxels, draw_order_position) = voxel_instances(&source, voxel_size);
        let voxel_options = VoxelGridOptions {
            world_from_grid: glam::Affine3A::IDENTITY,
            draw_order_position,
            voxel_size,
            picking_object_id: PickingLayerObjectId(1),
            outline_mask_ids: OutlineMaskPreference::NONE,
            depth_offset: 0,
        };
        let _ = VoxelGridDrawData::new(&voxel_ctx, &voxels, voxel_options)
            .expect("Failed to warm up voxel draw data");
        finish_frame(&mut voxel_ctx);

        group.throughput(Throughput::Bytes(
            (len * VoxelGridDrawData::gpu_instance_size_bytes()) as u64,
        ));
        group.bench_function(format!("voxel_grid_map_build_upload/{len}"), |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                for _ in 0..iters {
                    let (voxels, draw_order_position) =
                        voxel_instances(black_box(&source), voxel_size);
                    let draw_data = VoxelGridDrawData::new(
                        &voxel_ctx,
                        &voxels,
                        VoxelGridOptions {
                            draw_order_position,
                            ..voxel_options
                        },
                    )
                    .expect("Failed to create voxel draw data");
                    black_box(draw_data);
                    finish_frame(&mut voxel_ctx);
                }
                start.elapsed()
            });
        });

        let mut mesh_ctx = render_context();
        let cube_mesh = cube_gpu_mesh(&mesh_ctx);
        finish_frame(&mut mesh_ctx);
        let (meshes, bbox) = mesh_instances(&source, &cube_mesh);
        black_box(bbox);
        let _ = MeshDrawData::new(&mesh_ctx, &meshes).expect("Failed to warm up mesh draw data");
        finish_frame(&mut mesh_ctx);

        group.throughput(Throughput::Bytes(
            (len * MeshDrawData::gpu_instance_size_bytes()) as u64,
        ));
        group.bench_function(format!("boxes3d_default_mesh_build_upload/{len}"), |b| {
            b.iter_custom(|iters| {
                let start = Instant::now();
                for _ in 0..iters {
                    let (meshes, bbox) = mesh_instances(black_box(&source), &cube_mesh);
                    black_box(bbox);
                    let draw_data = MeshDrawData::new(&mesh_ctx, &meshes)
                        .expect("Failed to create mesh draw data");
                    black_box(draw_data);
                    finish_frame(&mut mesh_ctx);
                }
                start.elapsed()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_voxel_grid_draw_data);
criterion_main!(benches);
