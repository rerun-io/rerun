//! This example demonstrates how to use the Rerun Rust SDK to construct and log raw 3D meshes
//! (so-called "triangle soups") programmatically from scratch.
//!
//! It generates several geometric primitives, each demonstrating different `Mesh3D` features:
//! - Cube: per-vertex colors
//! - Pyramid: UV texture coordinates with a procedural checkerboard texture
//! - Sphere: vertex normals for smooth shading
//! - Icosahedron: flat shading (no normals)
//!
//! If you want to log existing mesh files (like GLTF, OBJ, STL, etc.), use the
//! [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d) archetype instead.
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh
//! ```

use std::f32::consts::PI;

use rerun::{Color, Mesh3D, RecordingStream, Rgba32, Transform3D, external::re_log};

// --- Mesh primitive structure ---

struct MeshPrimitive {
    vertex_positions: Vec<[f32; 3]>,
    vertex_colors: Option<Vec<Color>>,
    vertex_normals: Option<Vec<[f32; 3]>>,
    vertex_texcoords: Option<Vec<[f32; 2]>>,
    triangle_indices: Vec<u32>,
    albedo_factor: Option<[f32; 4]>,
    albedo_texture: Option<Vec<u8>>,
    texture_width: Option<u32>,
    texture_height: Option<u32>,
}

impl From<MeshPrimitive> for Mesh3D {
    fn from(primitive: MeshPrimitive) -> Self {
        let MeshPrimitive {
            vertex_positions,
            vertex_colors,
            vertex_normals,
            vertex_texcoords,
            triangle_indices,
            albedo_factor,
            albedo_texture,
            texture_width,
            texture_height,
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
                [255_u8, 200, 50] // Gold
            } else {
                [50_u8, 50, 50] // Dark gray
            };
            texture_data.extend_from_slice(&color);
        }
    }

    texture_data
}

/// Generate a cube with per-vertex colors.
///
/// Each face has a different color, demonstrating per-vertex coloring.
fn generate_cube() -> MeshPrimitive {
    // For proper per-face coloring, we need separate vertices for each face (24 vertices total)
    let vertex_positions = vec![
        // Front face (z = 0.5)
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
        // Back face (z = -0.5)
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [0.5, 0.5, -0.5],
        // Right face (x = 0.5)
        [0.5, -0.5, 0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [0.5, 0.5, 0.5],
        // Left face (x = -0.5)
        [-0.5, -0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [-0.5, 0.5, -0.5],
        // Top face (y = 0.5)
        [-0.5, 0.5, 0.5],
        [0.5, 0.5, 0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        // Bottom face (y = -0.5)
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, -0.5, 0.5],
        [-0.5, -0.5, 0.5],
    ];

    // Colors for each face (4 vertices per face with same color)
    let face_colors: [[u8; 3]; 6] = [
        [255, 100, 100], // Front - red
        [100, 255, 100], // Back - green
        [100, 100, 255], // Right - blue
        [255, 255, 100], // Left - yellow
        [255, 100, 255], // Top - magenta
        [100, 255, 255], // Bottom - cyan
    ];
    let vertex_colors: Vec<Color> = face_colors
        .iter()
        .flat_map(|c| {
            std::iter::repeat_n(Color::from_unmultiplied_rgba(c[0], c[1], c[2], 255), 4)
        })
        .collect();

    // Two triangles per face (counter-clockwise winding)
    let mut triangle_indices = Vec::new();
    for face in 0..6 {
        let base = face * 4;
        triangle_indices.extend_from_slice(&[base, base + 1, base + 2]);
        triangle_indices.extend_from_slice(&[base, base + 2, base + 3]);
    }

    MeshPrimitive {
        vertex_positions,
        vertex_colors: Some(vertex_colors),
        vertex_normals: None,
        vertex_texcoords: None,
        triangle_indices,
        albedo_factor: None,
        albedo_texture: None,
        texture_width: None,
        texture_height: None,
    }
}

/// Generate a pyramid with UV texture coordinates.
///
/// A four-sided pyramid with a square base, suitable for demonstrating texture mapping.
fn generate_pyramid() -> MeshPrimitive {
    let apex = [0.0_f32, 0.7, 0.0];
    let base_corners: [[f32; 3]; 4] = [
        [-0.5, -0.3, -0.5], // back-left
        [0.5, -0.3, -0.5],  // back-right
        [0.5, -0.3, 0.5],   // front-right
        [-0.5, -0.3, 0.5],  // front-left
    ];

    let mut vertex_positions = Vec::new();
    let mut vertex_texcoords = Vec::new();
    let mut triangle_indices = Vec::new();

    // Four triangular faces
    for i in 0..4 {
        let next_i = (i + 1) % 4;
        let base_idx = vertex_positions.len() as u32;

        vertex_positions.push(apex);
        vertex_positions.push(base_corners[i]);
        vertex_positions.push(base_corners[next_i]);

        // UV coordinates: apex at top center, base corners at bottom
        vertex_texcoords.push([0.5, 0.0]); // apex
        vertex_texcoords.push([0.0, 1.0]); // base corner 1
        vertex_texcoords.push([1.0, 1.0]); // base corner 2

        // Triangle (counter-clockwise when viewed from outside)
        triangle_indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 1]);
    }

    // Base (two triangles)
    let base_start = vertex_positions.len() as u32;
    for corner in &base_corners {
        vertex_positions.push(*corner);
    }
    // UV for base
    vertex_texcoords.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    // Base triangles (counter-clockwise when viewed from below)
    triangle_indices.extend_from_slice(&[base_start, base_start + 1, base_start + 2]);
    triangle_indices.extend_from_slice(&[base_start, base_start + 2, base_start + 3]);

    MeshPrimitive {
        vertex_positions,
        vertex_colors: None,
        vertex_normals: None,
        vertex_texcoords: Some(vertex_texcoords),
        triangle_indices,
        albedo_factor: None,
        albedo_texture: None,
        texture_width: None,
        texture_height: None,
    }
}

/// Generate a UV sphere with vertex normals for smooth shading.
///
/// Creates a sphere using latitude/longitude parameterization.
fn generate_sphere(subdivisions: u32) -> MeshPrimitive {
    debug_assert!(subdivisions >= 3, "Sphere subdivisions must be at least 3");

    let lat_divs = subdivisions as usize;
    let lon_divs = (subdivisions * 2) as usize;

    let mut vertex_positions = Vec::new();
    let mut vertex_normals = Vec::new();

    // Generate vertices and normals
    for lat in 0..=lat_divs {
        let theta = PI * lat as f32 / lat_divs as f32; // 0 to pi
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..lon_divs {
            let phi = 2.0 * PI * lon as f32 / lon_divs as f32; // 0 to 2pi
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            // Position on unit sphere
            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            vertex_positions.push([x * 0.5, y * 0.5, z * 0.5]); // Scale to radius 0.5
            vertex_normals.push([x, y, z]); // Normal is same as position direction for unit sphere
        }
    }

    // Generate triangle indices
    let mut triangle_indices = Vec::new();
    for lat in 0..lat_divs {
        for lon in 0..lon_divs {
            let next_lon = (lon + 1) % lon_divs;

            let first = (lat * lon_divs + lon) as u32;
            let first_next = (lat * lon_divs + next_lon) as u32;
            let second = ((lat + 1) * lon_divs + lon) as u32;
            let second_next = ((lat + 1) * lon_divs + next_lon) as u32;

            // Two triangles per quad (counter-clockwise winding)
            triangle_indices.extend_from_slice(&[first, second, first_next]);
            triangle_indices.extend_from_slice(&[first_next, second, second_next]);
        }
    }

    MeshPrimitive {
        vertex_positions,
        vertex_colors: None,
        vertex_normals: Some(vertex_normals),
        vertex_texcoords: None,
        triangle_indices,
        albedo_factor: None,
        albedo_texture: None,
        texture_width: None,
        texture_height: None,
    }
}

/// Generate an icosahedron (20-sided regular polyhedron) for flat shading.
///
/// The icosahedron is rendered without vertex normals, resulting in flat-shaded faces.
fn generate_icosahedron() -> MeshPrimitive {
    // Golden ratio
    let phi = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let scale = 0.3; // Scale to reasonable size

    // 12 vertices of an icosahedron
    let raw_vertices: [[f32; 3]; 12] = [
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ];

    // Normalize to unit sphere and scale
    let norm = (1.0 + phi * phi).sqrt();
    let vertex_positions: Vec<[f32; 3]> = raw_vertices
        .iter()
        .map(|v| [v[0] / norm * scale, v[1] / norm * scale, v[2] / norm * scale])
        .collect();

    // 20 triangular faces (counter-clockwise winding)
    let triangle_indices = vec![
        0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7,
        1, 8, 3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9,
        8, 1,
    ];

    MeshPrimitive {
        vertex_positions,
        vertex_colors: None,
        vertex_normals: None,
        vertex_texcoords: None,
        triangle_indices,
        albedo_factor: None,
        albedo_texture: None,
        texture_width: None,
        texture_height: None,
    }
}

// --- Main ---

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
    anyhow::ensure!(
        args.sphere_subdivisions >= 3,
        "--sphere-subdivisions must be at least 3"
    );

    // Set coordinate system (right-handed, Y-up)
    rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Y_UP())?;

    // --- Cube with per-vertex colors ---
    re_log::info!("Logging cube with vertex colors...");
    let cube = generate_cube();
    rec.log("world/cube", &Transform3D::from_translation([-1.5, 0.0, 0.0]))?;
    rec.log("world/cube", &Mesh3D::from(cube))?;

    // --- Pyramid with texture ---
    re_log::info!("Logging pyramid with texture...");
    let mut pyramid = generate_pyramid();
    let texture_data = generate_checkerboard_texture(64, 8);
    pyramid.albedo_texture = Some(texture_data);
    pyramid.texture_width = Some(64);
    pyramid.texture_height = Some(64);
    rec.log(
        "world/pyramid",
        &Transform3D::from_translation([1.5, 0.0, 0.0]),
    )?;
    rec.log("world/pyramid", &Mesh3D::from(pyramid))?;

    // --- Sphere with vertex normals (smooth shading) ---
    re_log::info!(
        subdivisions = args.sphere_subdivisions,
        "Logging sphere with vertex normals..."
    );
    let mut sphere = generate_sphere(args.sphere_subdivisions);
    sphere.albedo_factor = Some([100.0 / 255.0, 150.0 / 255.0, 1.0, 1.0]); // Light blue
    rec.log(
        "world/sphere",
        &Transform3D::from_translation([0.0, 0.0, 1.5]),
    )?;
    rec.log("world/sphere", &Mesh3D::from(sphere))?;

    // --- Icosahedron (flat shading, no normals) ---
    re_log::info!("Logging icosahedron (flat shaded)...");
    let mut icosahedron = generate_icosahedron();
    icosahedron.albedo_factor = Some([1.0, 180.0 / 255.0, 100.0 / 255.0, 1.0]); // Orange
    rec.log(
        "world/icosahedron",
        &Transform3D::from_translation([0.0, 0.0, -1.5]),
    )?;
    rec.log("world/icosahedron", &Mesh3D::from(icosahedron))?;

    re_log::info!("Done! All meshes logged to Rerun.");

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_raw_mesh")?;
    run(&rec, &args)
}
