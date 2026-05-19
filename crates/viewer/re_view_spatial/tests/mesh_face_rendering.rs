use re_log_types::TimePoint;
use re_sdk_types::RowId;
use re_sdk_types::archetypes::Mesh3D;
use re_sdk_types::components::{AlbedoFactor, MeshFaceRendering};
use re_sdk_types::datatypes::Rgba32;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

/// Creates a tetrahedron mesh with rainbow vertex colors.
fn rainbow_tetrahedron(
    face_rendering: MeshFaceRendering,
    albedo_factor: Option<AlbedoFactor>,
) -> Mesh3D {
    // Regular tetrahedron vertices.
    let vertices: [[f32; 3]; 4] = [
        [0.0, 1.0, 0.0],   // top
        [-1.0, -0.5, 0.5], // front-left
        [1.0, -0.5, 0.5],  // front-right
        [0.0, -0.5, -0.8], // back
    ];

    // 4 faces, wound counter-clockwise when viewed from outside.
    let indices: [[u32; 3]; 4] = [
        [0, 2, 1], // front
        [0, 3, 2], // right
        [0, 1, 3], // left
        [1, 2, 3], // bottom
    ];

    // Rainbow vertex colors (RGBA as u32, 0xRRGGBBAA).
    let colors: [u32; 4] = [
        0xFF0000FF, // red
        0x00FF00FF, // green
        0x0000FFFF, // blue
        0xFFFF00FF, // yellow
    ];

    let mesh = Mesh3D::new(vertices)
        .with_triangle_indices(indices)
        .with_vertex_colors(colors)
        .with_face_rendering(face_rendering);

    if let Some(factor) = albedo_factor {
        mesh.with_albedo_factor(factor)
    } else {
        mesh
    }
}

fn run_mesh_face_rendering_test(
    face_rendering: MeshFaceRendering,
    albedo_factor: Option<AlbedoFactor>,
    snapshot_name: &str,
) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("world/mesh", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &rainbow_tetrahedron(face_rendering, albedo_factor),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView::root(),
        );
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);
        view_id
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.snapshot(snapshot_name);
}

/// Semi-transparent albedo factor: white color with 25% alpha.
const SEMI_TRANSPARENT: AlbedoFactor = AlbedoFactor(Rgba32(0xFFFFFF40));

#[test]
pub fn test_mesh_face_rendering_double_sided_opaque() {
    run_mesh_face_rendering_test(
        MeshFaceRendering::DoubleSided,
        None,
        "mesh_face_rendering_double_sided_opaque",
    );
}

#[test]
pub fn test_mesh_face_rendering_back_opaque() {
    run_mesh_face_rendering_test(
        MeshFaceRendering::Back,
        None,
        "mesh_face_rendering_back_opaque",
    );
}

#[test]
pub fn test_mesh_face_rendering_front_opaque() {
    run_mesh_face_rendering_test(
        MeshFaceRendering::Front,
        None,
        "mesh_face_rendering_front_opaque",
    );
}

#[test]
pub fn test_mesh_face_rendering_double_sided_transparent() {
    run_mesh_face_rendering_test(
        MeshFaceRendering::DoubleSided,
        Some(SEMI_TRANSPARENT),
        "mesh_face_rendering_double_sided_transparent",
    );
}

#[test]
pub fn test_mesh_face_rendering_back_transparent() {
    run_mesh_face_rendering_test(
        MeshFaceRendering::Back,
        Some(SEMI_TRANSPARENT),
        "mesh_face_rendering_back_transparent",
    );
}

#[test]
pub fn test_mesh_face_rendering_front_transparent() {
    run_mesh_face_rendering_test(
        MeshFaceRendering::Front,
        Some(SEMI_TRANSPARENT),
        "mesh_face_rendering_front_transparent",
    );
}
