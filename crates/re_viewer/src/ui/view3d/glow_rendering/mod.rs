mod sphere_renderer;

use super::{eye::Eye, mesh_cache::GpuMeshCache, scene::*};

type LineMaterial = three_d::ColorMaterial;

type ThreeDResult<T> = Result<T, three_d::CoreError>;

pub struct RenderingContext {
    three_d: three_d::Context,
    skybox_dark: three_d::Skybox,
    skybox_light: three_d::Skybox,
    ambient_dark: three_d::AmbientLight,
    ambient_light: three_d::AmbientLight,

    gpu_scene: GpuScene,
}

impl RenderingContext {
    pub fn new(gl: &std::sync::Arc<glow::Context>) -> ThreeDResult<Self> {
        let three_d = three_d::Context::from_gl_context(gl.clone())?;

        let skybox_dark = load_skybox_texture(&three_d, skybox_dark);
        let skybox_light = load_skybox_texture(&three_d, skybox_light);

        let ambient_light_intensity = 5.0;
        let ambient_dark = three_d::AmbientLight::new_with_environment(
            &three_d,
            ambient_light_intensity,
            three_d::Color::WHITE,
            skybox_dark.texture(),
        );
        let ambient_light = three_d::AmbientLight::new_with_environment(
            &three_d,
            ambient_light_intensity,
            three_d::Color::WHITE,
            skybox_light.texture(),
        );

        let gpu_scene = GpuScene::new(&three_d);

        Ok(Self {
            three_d,
            skybox_dark,
            skybox_light,
            ambient_dark,
            ambient_light,
            gpu_scene,
        })
    }
}

pub struct GpuScene {
    gpu_meshes: GpuMeshCache,
    points: sphere_renderer::InstancedSpheres<three_d::PhysicalMaterial>,
    lines: three_d::Gm<three_d::InstancedMesh, LineMaterial>,

    mesh_instances: std::collections::HashMap<u64, three_d::Instances>,
}

impl GpuScene {
    pub fn new(three_d: &three_d::Context) -> Self {
        let points_cache = sphere_renderer::InstancedSpheres::new_with_material(
            three_d,
            Default::default(),
            &three_d::CpuMesh::sphere(3), // fast
            default_material(),
        );

        let lines_cache = three_d::Gm::new(
            three_d::InstancedMesh::new(
                three_d,
                &Default::default(),
                &three_d::CpuMesh::cylinder(10),
            ),
            Default::default(),
        );

        Self {
            gpu_meshes: Default::default(),
            points: points_cache,
            lines: lines_cache,

            mesh_instances: Default::default(),
        }
    }

    pub fn set(&mut self, three_d: &three_d::Context, scene: &Scene) {
        crate::profile_function!();

        let Scene {
            points,
            line_segments,
            meshes,
            labels: _,
        } = scene;

        self.points.set_instances(allocate_points(points));

        self.lines
            .set_instances(&allocate_line_segments(line_segments));

        self.mesh_instances.clear();

        for mesh in meshes {
            let instances = self.mesh_instances.entry(mesh.mesh_id).or_default();

            let (scale, rotation, translation) =
                mesh.world_from_mesh.to_scale_rotation_translation();
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
            instances.colors.get_or_insert_with(Default::default).push(
                mesh.tint.map_or(three_d::Color::WHITE, |[r, g, b, a]| {
                    three_d::Color::new(r, g, b, a)
                }),
            );

            self.gpu_meshes.load(three_d, mesh.mesh_id, &mesh.cpu_mesh);
        }

        for (mesh_id, instances) in &self.mesh_instances {
            self.gpu_meshes.set_instances(*mesh_id, instances);
        }
    }

    pub fn collect_objects<'a>(&'a self, objects: &mut Vec<&'a dyn three_d::Object>) {
        crate::profile_function!();

        if self.points.instance_count() > 0 {
            objects.push(&self.points);
        }

        if self.lines.instance_count() > 0 {
            objects.push(&self.lines);
        }

        for &mesh_id in self.mesh_instances.keys() {
            if let Some(gpu_mesh) = self.gpu_meshes.get(mesh_id) {
                for obj in &gpu_mesh.meshes {
                    if obj.instance_count() > 0 {
                        objects.push(obj);
                    }
                }
            }
        }
    }
}

fn load_skybox_texture(
    three_d: &three_d::Context,
    color_from_dir: fn(glam::Vec3) -> [u8; 3],
) -> three_d::Skybox {
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

    three_d::Skybox::new(three_d, &a, &b, &c, &d, &e, &f)
}

fn generate_skybox_side(
    resolution: usize,
    color_from_dir: fn(glam::Vec3) -> [u8; 3],
    center_dir: glam::Vec3,
    x_dir: glam::Vec3,
    y_dir: glam::Vec3,
) -> three_d::CpuTexture {
    let data: Vec<[u8; 3]> = (0..resolution)
        .flat_map(|y| {
            let ty = egui::remap_clamp(y as f32, 0.0..=(resolution as f32 - 1.0), -1.0..=1.0);
            (0..resolution).map(move |x| {
                let tx = egui::remap_clamp(x as f32, 0.0..=(resolution as f32 - 1.0), -1.0..=1.0);
                let dir = center_dir + tx * x_dir + ty * y_dir;
                let dir = dir.normalize();
                color_from_dir(dir)
            })
        })
        .collect();

    three_d::CpuTexture {
        data: three_d::TextureData::RgbU8(data),
        width: resolution as _,
        height: resolution as _,
        wrap_s: three_d::Wrapping::ClampToEdge,
        wrap_t: three_d::Wrapping::ClampToEdge,
        ..Default::default()
    }
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
        is_transparent: true,
        render_states: RenderStates {
            blend: Blend::TRANSPARENCY,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn allocate_points(points: &[Point3D]) -> sphere_renderer::SphereInstances {
    crate::profile_function!();
    use three_d::*;

    let mut translations_and_scale = Vec::with_capacity(points.len());
    let mut colors = Vec::with_capacity(points.len());

    for point in points {
        let p = point.pos;
        let radius = point
            .radius
            .scene()
            .expect("size should have been translated to scene-coordinates");
        translations_and_scale.push(vec4(p[0], p[1], p[2], radius));
        colors.push(color_to_three_d(point.color));
    }

    sphere_renderer::SphereInstances {
        translations_and_scale,
        colors: Some(colors),
    }
}

fn allocate_line_segments(line_segments: &[LineSegments3D]) -> three_d::Instances {
    crate::profile_function!();
    use three_d::*;

    let mut translations = vec![];
    let mut rotations = vec![];
    let mut scales = vec![];
    let mut colors = vec![];

    for line_segments in line_segments {
        let LineSegments3D {
            instance_id_hash: _,
            segments,
            radius,
            color,
            ..
        } = line_segments;
        let radius = radius
            .scene()
            .expect("size should have been translated to scene-coordinates");

        for &(start, end) in segments {
            rotations.push(three_d::Quat::from(mint::Quaternion::from(
                glam::Quat::from_rotation_arc(glam::Vec3::X, (end - start).normalize()),
            )));

            translations.push(vec3(start.x, start.y, start.z));
            scales.push(vec3((start - end).length(), radius, radius));
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
    eye: &Eye,
    info: &egui::PaintCallbackInfo,
    scene: &Scene,
    dark_mode: bool,
    show_axes: bool, // TODO(emilk): less bool arguments
    painter: &egui_glow::Painter,
) {
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
    let scissor_box = ScissorBox {
        x: clip_rect.left_px.round() as _,
        y: clip_rect.from_bottom_px.round() as _,
        width: clip_rect.width_px.round() as _,
        height: clip_rect.height_px.round() as _,
    };
    three_d.set_blend(three_d::Blend::TRANSPARENCY);

    let position = eye.world_from_view.translation();
    let target = eye.world_from_view.transform_point3(-glam::Vec3::Z);
    let up = eye.world_from_view.transform_vector3(glam::Vec3::Y);
    let far = 100_000.0; // TODO(emilk): infinity (https://github.com/rustgd/cgmath/pull/547)
    let three_d_camera = three_d::Camera::new_perspective(
        viewport,
        mint::Vector3::from(position).into(),
        mint::Vector3::from(target).into(),
        mint::Vector3::from(up).into(),
        radians(eye.fov_y),
        eye.near(),
        far,
    );

    // -------------------

    let ambient = if dark_mode {
        &rendering.ambient_dark
    } else {
        &rendering.ambient_light
    };
    let directional0 = DirectionalLight::new(three_d, 5.0, Color::WHITE, &vec3(-1.0, -1.0, -1.0));
    let directional1 = DirectionalLight::new(three_d, 5.0, Color::WHITE, &vec3(1.0, 1.0, 1.0));
    let lights: &[&dyn Light] = &[ambient, &directional0, &directional1];

    // -------------------

    rendering.gpu_scene.set(three_d, scene);

    // -------------------

    let mut objects: Vec<&dyn Object> = vec![];

    if dark_mode {
        objects.push(&rendering.skybox_dark);
    } else {
        objects.push(&rendering.skybox_light);
    }

    let axes = three_d::Axes::new(three_d, 0.01, 1.0);
    if show_axes {
        objects.push(&axes);
    }

    rendering.gpu_scene.collect_objects(&mut objects);

    let (width, height) = (info.viewport.width() as u32, info.viewport.height() as u32);

    let render_target = painter.intermediate_fbo().map_or_else(
        || RenderTarget::screen(three_d, width, height),
        |fbo| RenderTarget::from_framebuffer(three_d, width, height, fbo),
    );

    crate::profile_scope!("render");
    render_target.render_partially(scissor_box, &three_d_camera, &objects, lights);
}

fn color_to_three_d([r, g, b, a]: [u8; 4]) -> three_d::Color {
    three_d::Color { r, g, b, a }
}

/// We get a [`glow::Context`] from `eframe`, but we want a [`three_d::Context`].
///
/// Sadly we can't just create and store a [`three_d::Context`] in the app and pass it
/// to the [`egui::PaintCallback`] because [`three_d::Context`] isn't `Send+Sync`, which
/// [`egui::PaintCallback`] is.
pub fn with_three_d_context<R>(
    gl: &std::sync::Arc<glow::Context>,
    f: impl FnOnce(&mut RenderingContext) -> R,
) -> R {
    use std::cell::RefCell;
    thread_local! {
        static THREE_D: RefCell<Option<RenderingContext>> = RefCell::new(None);
    }

    #[allow(unsafe_code)]
    // SAFETY: we should have a valid glow context here, and we _should_ be in the correct thread.
    unsafe {
        use glow::HasContext as _;
        gl.enable(glow::DEPTH_TEST);
        if !cfg!(target_arch = "wasm32") {
            gl.disable(glow::FRAMEBUFFER_SRGB);
        }
        gl.clear(glow::DEPTH_BUFFER_BIT);
    }

    THREE_D.with(|three_d| {
        let mut three_d = three_d.borrow_mut();
        let three_d = three_d.get_or_insert_with(|| RenderingContext::new(gl).unwrap());
        f(three_d)
    })
}
