//! This example demonstrates how to use the Rerun Rust SDK to construct and log raw 3D meshes
//! (so-called "triangle soups") programmatically from scratch, including their transform hierarchy.
//!
//! This example shows how to create geometric primitives by manually defining vertices, normals,
//! colors, texture coordinates, and materials.
//!
//! If you want to log existing mesh files (like GLTF, OBJ, STL, etc.), use the
//! [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d) archetype instead.
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh
//! ```

use std::f32::consts::PI;

use anyhow::ensure;
use rerun::{
    Color, Mesh3D, RecordingStream, Rgba32, RotationAxisAngle, Transform3D, external::re_log,
};

// --- Mesh primitive structures ---

#[derive(Clone)]
struct MeshPrimitive {
    albedo_factor: Option<[f32; 4]>,
    albedo_texture: Option<Vec<u8>>,
    texture_width: Option<u32>,
    texture_height: Option<u32>,
    vertex_positions: Vec<[f32; 3]>,
    vertex_colors: Option<Vec<Color>>,
    vertex_normals: Option<Vec<[f32; 3]>>,
    vertex_texcoords: Option<Vec<[f32; 2]>>,
    triangle_indices: Vec<u32>,
}

impl From<MeshPrimitive> for Mesh3D {
    fn from(primitive: MeshPrimitive) -> Self {
        let MeshPrimitive {
            albedo_factor,
            albedo_texture,
            texture_width,
            texture_height,
            vertex_positions,
            vertex_colors,
            vertex_normals,
            vertex_texcoords,
            triangle_indices,
        } = primitive;

        let mut mesh = Mesh3D::new(vertex_positions);

        // Convert flat indices to triangle tuples
        assert!(triangle_indices.len() % 3 == 0);
        let triangle_indices = triangle_indices
            .chunks_exact(3)
            .map(|tri| (tri[0], tri[1], tri[2]));
        mesh = mesh.with_triangle_indices(triangle_indices);

        if let Some(vertex_normals) = vertex_normals {
            mesh = mesh.with_vertex_normals(vertex_normals);
        }
        if let Some(vertex_colors) = vertex_colors {
            mesh = mesh.with_vertex_colors(vertex_colors);
        }
        if let Some(vertex_texcoords) = vertex_texcoords {
            mesh = mesh.with_vertex_texcoords(vertex_texcoords);
        }
        if let Some([r, g, b, a]) = albedo_factor {
            mesh = mesh.with_albedo_factor(Rgba32::from_linear_unmultiplied_rgba_f32(r, g, b, a));
        }
        if let Some(texture_data) = albedo_texture {
            if let (Some(width), Some(height)) = (texture_width, texture_height) {
                // Create ImageFormat for RGB8 texture
                let image_format = rerun::components::ImageFormat::rgb8([width, height]);
                mesh = mesh.with_albedo_texture(image_format, texture_data);
            }
        }

        mesh.sanity_check().unwrap();

        mesh
    }
}

// --- Geometric primitive generators ---

/// Generate a simple checkerboard texture.
fn generate_checkerboard_texture(size: u32, checker_size: u32) -> Vec<u8> {
    let mut texture_data = Vec::with_capacity((size * size * 3) as usize);

    for i in 0..size {
        for j in 0..size {
            let checker = ((i / checker_size) + (j / checker_size)) % 2 == 0;
            let color = if checker {
                [200_u8, 200, 200] // Light gray
            } else {
                [50_u8, 50, 50] // Dark gray
            };
            texture_data.extend_from_slice(&color);
        }
    }

    texture_data
}

/// Generate a UV sphere.
///
/// Creates a sphere using latitude/longitude parameterization.
/// Returns base geometry with positions, normals, UV coordinates, and per-vertex colors.
fn generate_sphere(subdivisions: u32) -> MeshPrimitive {
    debug_assert!(subdivisions >= 3, "Sphere subdivisions must be at least 3");

    let lat_divs = subdivisions as usize;
    let lon_divs = (subdivisions * 2) as usize;

    let mut vertex_positions = Vec::new();
    let mut vertex_normals = Vec::new();
    let mut vertex_texcoords = Vec::new();
    let mut vertex_colors = Vec::new();

    // Generate vertices, normals, UVs, and colors
    for lat in 0..=lat_divs {
        let theta = PI * lat as f32 / lat_divs as f32; // 0 to pi
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();
        let v = lat as f32 / lat_divs as f32; // V coordinate

        for lon in 0..lon_divs {
            let phi = 2.0 * PI * lon as f32 / lon_divs as f32; // 0 to 2pi
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();
            let u = lon as f32 / lon_divs as f32; // U coordinate

            // Position on unit sphere
            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            vertex_positions.push([x * 0.5, y * 0.5, z * 0.5]); // Scale to radius 0.5
            vertex_normals.push([x, y, z]); // Normal is same as position for unit sphere
            vertex_texcoords.push([u, v]);

            // Generate per-vertex colors based on position (creates a nice gradient)
            let r = (128.0 + 127.0 * x) as u8;
            let g = (128.0 + 127.0 * y) as u8;
            let b = (128.0 + 127.0 * z) as u8;
            vertex_colors.push(Color::from_unmultiplied_rgba(r, g, b, 255));
        }
    }

    // Generate triangle indices
    let mut triangle_indices = Vec::new();
    for lat in 0..lat_divs {
        let curr_row_start = lat * lon_divs;
        let next_row_start = (lat + 1) * lon_divs;

        for lon in 0..lon_divs {
            let next_lon = (lon + 1) % lon_divs;

            let first = (curr_row_start + lon) as u32;
            let first_next = (curr_row_start + next_lon) as u32;
            let second = (next_row_start + lon) as u32;
            let second_next = (next_row_start + next_lon) as u32;

            // Two triangles per quad (counter-clockwise winding)
            triangle_indices.extend_from_slice(&[first, second, first_next]);
            triangle_indices.extend_from_slice(&[first_next, second, second_next]);
        }
    }

    MeshPrimitive {
        albedo_factor: None,
        albedo_texture: None,
        texture_width: None,
        texture_height: None,
        vertex_positions,
        vertex_colors: Some(vertex_colors),
        vertex_normals: Some(vertex_normals),
        vertex_texcoords: Some(vertex_texcoords),
        triangle_indices,
    }
}

// --- Init ---

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Number of subdivisions for the sphere (default: 32)
    #[clap(long, default_value = "32")]
    sphere_subdivisions: u32,
}

fn run(rec: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    ensure!(
        args.sphere_subdivisions >= 3,
        "--sphere-subdivisions must be at least 3"
    );

    // Set coordinate system (right-handed, Y-up)
    rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Y_UP())?;

    // Generate base sphere geometry once
    re_log::info!(
        subdivisions = args.sphere_subdivisions,
        "Generating sphere..."
    );
    let sphere = generate_sphere(args.sphere_subdivisions);

    // Instance 1: Vertex colors only (center)
    re_log::info!("Logging sphere with vertex colors...");
    rec.log(
        "world/sphere/vertex_colors",
        &Transform3D::from_translation([0.0, 0.0, 0.0]),
    )?;
    rec.log(
        "world/sphere/vertex_colors",
        &Mesh3D::from(MeshPrimitive {
            vertex_colors: sphere.vertex_colors.clone(),
            vertex_positions: sphere.vertex_positions.clone(),
            triangle_indices: sphere.triangle_indices.clone(),
            albedo_factor: None,
            albedo_texture: None,
            texture_width: None,
            texture_height: None,
            vertex_normals: None,
            vertex_texcoords: None,
        }),
    )?;

    // Instance 2: Albedo factor (solid color, left)
    re_log::info!("Logging sphere with albedo factor...");
    rec.log(
        "world/sphere/albedo_factor",
        &Transform3D::from_translation([-1.5, 0.0, 0.0]),
    )?;
    rec.log(
        "world/sphere/albedo_factor",
        &Mesh3D::from(MeshPrimitive {
            albedo_factor: Some([1.0, 100.0 / 255.0, 150.0 / 255.0, 1.0]), // Pink
            vertex_positions: sphere.vertex_positions.clone(),
            triangle_indices: sphere.triangle_indices.clone(),
            albedo_texture: None,
            texture_width: None,
            texture_height: None,
            vertex_colors: None,
            vertex_normals: None,
            vertex_texcoords: None,
        }),
    )?;

    // Instance 3: Albedo texture with UV coordinates (right)
    re_log::info!("Logging sphere with albedo texture...");
    let texture_data = generate_checkerboard_texture(32, 4);
    rec.log(
        "world/sphere/albedo_texture",
        &Transform3D::from_translation([1.5, 0.0, 0.0]),
    )?;
    rec.log(
        "world/sphere/albedo_texture",
        &Mesh3D::from(MeshPrimitive {
            albedo_texture: Some(texture_data),
            texture_width: Some(32),
            texture_height: Some(32),
            vertex_texcoords: sphere.vertex_texcoords.clone(),
            vertex_positions: sphere.vertex_positions.clone(),
            triangle_indices: sphere.triangle_indices.clone(),
            albedo_factor: None,
            vertex_colors: None,
            vertex_normals: None,
        }),
    )?;

    // Instance 4: Vertex normals for smooth shading (above)
    re_log::info!("Logging sphere with vertex normals...");
    rec.log(
        "world/sphere/vertex_normals",
        &Transform3D::from_translation([0.0, 1.5, 0.0]),
    )?;
    rec.log(
        "world/sphere/vertex_normals",
        &Mesh3D::from(MeshPrimitive {
            vertex_normals: sphere.vertex_normals.clone(),
            albedo_factor: Some([100.0 / 255.0, 150.0 / 255.0, 1.0, 1.0]), // Light blue
            vertex_positions: sphere.vertex_positions.clone(),
            triangle_indices: sphere.triangle_indices.clone(),
            albedo_texture: None,
            texture_width: None,
            texture_height: None,
            vertex_colors: None,
            vertex_texcoords: None,
        }),
    )?;

    re_log::info!("Done! All mesh variations logged to Rerun.");

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_raw_mesh")?;
    run(&rec, &args)
}
