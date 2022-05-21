use super::scene::*;
use super::{camera::Camera, MeshCache};

type LineMaterial = three_d::ColorMaterial;

pub struct RenderingContext {
    three_d: three_d::Context,
    skybox_dark: three_d::Skybox,
    skybox_light: three_d::Skybox,
    ambient_dark: three_d::AmbientLight,
    ambient_light: three_d::AmbientLight,

    mesh_cache: MeshCache,

    /// So we don't need to re-allocate them.
    points_cache: three_d::InstancedModel<three_d::PhysicalMaterial>,
    lines_cache: three_d::InstancedModel<LineMaterial>,
}

impl RenderingContext {
    pub fn new(gl: &std::rc::Rc<glow::Context>) -> three_d::ThreeDResult<Self> {
        let three_d = three_d::Context::from_gl_context(gl.clone())?;

        let skybox_dark =
            three_d::Skybox::new(&three_d, &load_skybox_texture(skybox_dark)).unwrap();
        let skybox_light =
            three_d::Skybox::new(&three_d, &load_skybox_texture(skybox_light)).unwrap();

        let intensity = 1.0;
        let ambient_dark = three_d::AmbientLight::new_with_environment(
            &three_d,
            intensity,
            three_d::Color::WHITE,
            skybox_dark.texture(),
        )
        .unwrap();
        let ambient_light = three_d::AmbientLight::new_with_environment(
            &three_d,
            intensity,
            three_d::Color::WHITE,
            skybox_light.texture(),
        )
        .unwrap();

        let sphere_mesh = three_d::CpuMesh::sphere(24);
        let points_cache = three_d::InstancedModel::new_with_material(
            &three_d,
            &three_d::Instances {
                // we must declare what we intend to use:
                translations: vec![],
                scales: Some(vec![]),
                colors: Some(vec![]),
                ..Default::default()
            },
            &sphere_mesh,
            default_material(),
        )
        .unwrap();

        let line_mesh = three_d::CpuMesh::cylinder(10);
        let lines_cache = three_d::InstancedModel::new_with_material(
            &three_d,
            &three_d::Instances {
                // we must declare what we intend to use:
                translations: vec![],
                rotations: Some(vec![]),
                scales: Some(vec![]),
                colors: Some(vec![]),
                ..Default::default()
            },
            &line_mesh,
            Default::default(),
        )
        .unwrap();

        Ok(Self {
            three_d,
            skybox_dark,
            skybox_light,
            ambient_dark,
            ambient_light,
            mesh_cache: Default::default(),
            points_cache,
            lines_cache,
        })
    }
}

fn load_skybox_texture(color_from_dir: fn(glam::Vec3) -> [u8; 3]) -> three_d::CpuTextureCube {
    crate::profile_function!();

    let resolution = 64;

    use glam::Vec3;
    const X: Vec3 = Vec3::X;
    const Y: Vec3 = Vec3::Y;
    const Z: Vec3 = Vec3::Z;

    let a = generate_skybox_side(resolution, color_from_dir, X, -Z, -Y);
    let b = generate_skybox_side(resolution, color_from_dir, -X, Z, -Y);
    let c = generate_skybox_side(resolution, color_from_dir, Y, X, Z);
    let d = generate_skybox_side(resolution, color_from_dir, -Y, X, -Z);
    let e = generate_skybox_side(resolution, color_from_dir, Z, X, -Y);
    let f = generate_skybox_side(resolution, color_from_dir, -Z, -X, -Y);

    let data = three_d::TextureCubeData::RgbU8(a, b, c, d, e, f);

    three_d::CpuTextureCube {
        data,
        width: resolution as _,
        height: resolution as _,
        ..Default::default()
    }
}

fn generate_skybox_side(
    resolution: usize,
    color_from_dir: fn(glam::Vec3) -> [u8; 3],
    center_dir: glam::Vec3,
    x_dir: glam::Vec3,
    y_dir: glam::Vec3,
) -> Vec<[u8; 3]> {
    (0..resolution)
        .flat_map(|y| {
            let ty = egui::remap_clamp(y as f32, 0.0..=(resolution as f32 - 1.0), -1.0..=1.0);
            (0..resolution).map(move |x| {
                let tx = egui::remap_clamp(x as f32, 0.0..=(resolution as f32 - 1.0), -1.0..=1.0);
                let dir = center_dir + tx * x_dir + ty * y_dir;
                let dir = dir.normalize();
                color_from_dir(dir)
            })
        })
        .collect()
}

/// Color from view direction
fn skybox_dark(dir: glam::Vec3) -> [u8; 3] {
    let rgb = dir * 0.5 + glam::Vec3::splat(0.5); // 0-1 range
    let rgb = glam::Vec3::splat(0.05) + 0.20 * rgb;
    [
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
    ]
}

/// Color from view direction
fn skybox_light(dir: glam::Vec3) -> [u8; 3] {
    let rgb = dir * 0.5 + glam::Vec3::splat(0.5); // 0-1 range
    let rgb = glam::Vec3::splat(0.85) + 0.15 * rgb;
    [
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
    ]
}

fn default_material() -> three_d::PhysicalMaterial {
    use three_d::*;
    PhysicalMaterial {
        roughness: 1.0,
        metallic: 0.0,
        lighting_model: LightingModel::Cook(
            NormalDistributionFunction::TrowbridgeReitzGGX,
            GeometryFunction::SmithSchlickGGX,
        ),
        ..Default::default()
    }
}

fn allocate_points(points: &[Point]) -> three_d::Instances {
    crate::profile_function!();
    use three_d::*;

    let mut translations = vec![];
    let mut scales = vec![];
    let mut colors = vec![];

    for point in points {
        let p = point.pos;
        translations.push(vec3(p[0], p[1], p[2]));
        scales.push(vec3(point.radius, point.radius, point.radius));
        colors.push(color_to_three_d(point.color));
    }

    three_d::Instances {
        translations,
        scales: Some(scales),
        colors: Some(colors),
        ..Default::default()
    }
}

fn allocate_line_segments(line_segments: &[LineSegments]) -> three_d::Instances {
    crate::profile_function!();
    use three_d::*;

    let mut translations = vec![];
    let mut rotations = vec![];
    let mut scales = vec![];
    let mut colors = vec![];

    for line_segments in line_segments {
        let LineSegments {
            segments,
            radius,
            color,
        } = line_segments;

        for &[p0, p1] in segments {
            rotations.push(three_d::Quat::from(mint::Quaternion::from(
                glam::Quat::from_rotation_arc(
                    glam::Vec3::X,
                    (glam::Vec3::from(p1) - glam::Vec3::from(p0)).normalize(),
                ),
            )));

            let p0 = vec3(p0[0], p0[1], p0[2]);
            let p1 = vec3(p1[0], p1[1], p1[2]);
            translations.push(p0);
            scales.push(vec3((p0 - p1).magnitude(), *radius, *radius));
            colors.push(color_to_three_d(*color));
        }
    }

    three_d::Instances {
        translations,
        scales: Some(scales),
        rotations: Some(rotations),
        colors: Some(colors),
        ..Default::default()
    }
}

pub fn paint_with_three_d(
    rendering: &mut RenderingContext,
    camera: &Camera,
    info: &egui::PaintCallbackInfo,
    scene: &Scene,
    dark_mode: bool,
) -> three_d::ThreeDResult<()> {
    crate::profile_function!();
    use three_d::*;
    let three_d = &rendering.three_d;

    let viewport = info.viewport_in_pixels();
    let viewport = Viewport {
        x: viewport.left_px.round() as _,
        y: viewport.from_bottom_px.round() as _,
        width: viewport.width_px.round() as _,
        height: viewport.height_px.round() as _,
    };

    // Respect the egui clip region (e.g. if we are inside an `egui::ScrollArea`).
    let clip_rect = info.clip_rect_in_pixels();
    three_d.set_scissor(ScissorBox {
        x: clip_rect.left_px.round() as _,
        y: clip_rect.from_bottom_px.round() as _,
        width: clip_rect.width_px.round() as _,
        height: clip_rect.height_px.round() as _,
    });

    let position = camera.world_from_view.translation();
    let target = camera.world_from_view.transform_point3(-glam::Vec3::Z);
    let up = camera.world_from_view.transform_vector3(glam::Vec3::Y);
    let camera = Camera::new_perspective(
        three_d,
        viewport,
        mint::Vector3::from(position).into(),
        mint::Vector3::from(target).into(),
        mint::Vector3::from(up).into(),
        radians(camera.fov_y),
        camera.near(),
        1000.0, // TODO: infinity (https://github.com/rustgd/cgmath/pull/547)
    )?;

    // -------------------

    let ambient = if dark_mode {
        &rendering.ambient_dark
    } else {
        &rendering.ambient_light
    };
    let directional0 = DirectionalLight::new(three_d, 2.0, Color::WHITE, &vec3(-1.0, -1.0, -1.0))?;
    let directional1 = DirectionalLight::new(three_d, 2.0, Color::WHITE, &vec3(1.0, 1.0, 1.0))?;
    let lights: &[&dyn Light] = &[ambient, &directional0, &directional1];

    // -------------------

    let Scene {
        points,
        line_segments,
        meshes,
    } = scene;

    let mut mesh_instances: std::collections::HashMap<u64, Instances> = Default::default();

    for mesh in meshes {
        let instances = mesh_instances
            .entry(mesh.mesh_id)
            .or_insert_with(|| Instances {
                translations: vec![],
                ..Default::default()
            });

        let (scale, rotation, translation) = mesh.world_from_mesh.to_scale_rotation_translation();
        instances
            .translations
            .push(mint::Vector3::from(translation).into());
        instances
            .rotations
            .get_or_insert_with(Default::default)
            .push(mint::Quaternion::from(rotation).into());
        instances
            .scales
            .get_or_insert_with(Default::default)
            .push(mint::Vector3::from(scale).into());

        rendering
            .mesh_cache
            .load(three_d, mesh.mesh_id, &mesh.name, &mesh.mesh_data);
    }

    for (mesh_id, instances) in &mesh_instances {
        rendering.mesh_cache.set_instances(*mesh_id, instances)?;
    }

    let mut objects: Vec<&dyn Object> = vec![];

    if dark_mode {
        objects.push(&rendering.skybox_dark);
    } else {
        objects.push(&rendering.skybox_light);
    }

    // let axes = three_d::Axes::new(three_d, 0.5, 10.0).unwrap();
    // objects.push(&axes);

    for &mesh_id in mesh_instances.keys() {
        if let Some(gpu_mesh) = rendering.mesh_cache.get(mesh_id) {
            for obj in &gpu_mesh.models {
                objects.push(obj);
            }
        }
    }

    rendering
        .points_cache
        .set_instances(&allocate_points(points))?;
    objects.push(&rendering.points_cache);

    rendering
        .lines_cache
        .set_instances(&allocate_line_segments(line_segments))?;
    objects.push(&rendering.lines_cache);

    crate::profile_scope!("render_pass");
    render_pass(&camera, &objects, lights)?;

    Ok(())
}

/// TODO: don't use `Color32` here, it is unnecessary to premultiply alpha only to again unmultiply it here!
fn color_to_three_d(color: egui::Color32) -> three_d::Color {
    // TODO: figure out why three_d colors are messed up. Are they in linear space and in bytes!?!?!
    let [r, g, b, a] = egui::Rgba::from(color).to_rgba_unmultiplied();
    three_d::Color::new(
        (r * 255.0).round() as _,
        (g * 255.0).round() as _,
        (b * 255.0).round() as _,
        (a * 255.0).round() as _,
    )
}
