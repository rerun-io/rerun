#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/sphere_quad.wgsl>
#import <./utils/depth_offset.wgsl>

@group(1) @binding(0)
var position_data_texture: texture_2d<f32>;

@group(1) @binding(1)
var color_texture: texture_2d<f32>;

/// 3D scale of point splats.
@group(1) @binding(2)
var scale_texture: texture_2d<f32>;

/// XYZW quaternion of point splats.
@group(1) @binding(3)
var rotation_texture: texture_2d<f32>;

@group(1) @binding(4)
var picking_instance_id_texture: texture_2d<u32>;

struct DrawDataUniformBuffer {
    radius_boost_in_ui_points: f32,
    // In actuality there is way more padding than this since we align all our uniform buffers to
    // 256bytes in order to allow them to be buffer-suballocations.
    // However, wgpu doesn't know this at this point and therefore requires `DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`
    // if we wouldn't add padding here, which isn't available on WebGL.
    _padding: vec4f,
};
@group(1) @binding(5)
var<uniform> draw_data: DrawDataUniformBuffer;

struct BatchUniformBuffer {
    world_from_obj: mat4x4f,
    flags: u32,
    depth_offset: f32,
    _padding: vec2u,
    outline_mask: vec2u,
    picking_layer_object_id: vec2u,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

// Flags
// See point_cloud.rs#PointCloudBatchFlags
const FLAG_ENABLE_SHADING: u32 = 1u;
const FLAG_DRAW_AS_CIRCLES: u32 = 2u;

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(perspective)
    world_position: vec3f,

    @location(1) @interpolate(flat)
    radius: f32,

    @location(2) @interpolate(flat)
    point_center: vec3f,

    // TODO(andreas): Color & picking layer instance are only used in some passes.
    // Once we have shader variant support we should remove the unused ones
    // (it's unclear how good shader compilers are at removing unused outputs and associated texture fetches)
    // TODO(andreas): Is fetching color & picking layer in the fragment shader maybe more efficient?
    // Yes, that's more fetches but all of these would be cache hits whereas vertex data pass through can be expensive, (especially on tiler architectures!)

    @location(3) @interpolate(flat)
    color: vec4f, // linear RGBA with unmulitplied/separate alpha

    @location(4) @interpolate(flat)
    picking_instance_id: vec2u,

    // [-2, +2] coordinates on the point splat
    @location(5) @interpolate(perspective)
    vpos: vec2f,
};

struct PointData {
    pos: vec3f,
    unresolved_radius: f32,
    color: vec4f,
    scale: vec3f,
    rotation_quat_xyzw: vec4f,
    picking_instance_id: vec2u,
}

// Read and unpack data at a given location
fn read_data(idx: u32) -> PointData {
    let position_data_texture_size = textureDimensions(position_data_texture);
    let position_data = textureLoad(position_data_texture,
         vec2u(idx % position_data_texture_size.x, idx / position_data_texture_size.x), 0);

    let color_texture_size = textureDimensions(color_texture);
    let color = textureLoad(color_texture,
         vec2u(idx % color_texture_size.x, idx / color_texture_size.x), 0);

    let scale_texture_size = textureDimensions(scale_texture);
    let scale = textureLoad(scale_texture,
         vec2u(idx % scale_texture_size.x, idx / scale_texture_size.x), 0).xyz;

    let rotation_texture_size = textureDimensions(rotation_texture);
    let rotation = textureLoad(rotation_texture,
         vec2u(idx % rotation_texture_size.x, idx / rotation_texture_size.x), 0);

    let picking_instance_id_texture_size = textureDimensions(picking_instance_id_texture);
    let picking_instance_id = textureLoad(picking_instance_id_texture,
         vec2u(idx % picking_instance_id_texture_size.x, idx / picking_instance_id_texture_size.x), 0).xy;

    var data: PointData;
    let pos_4d = batch.world_from_obj * vec4f(position_data.xyz, 1.0);
    data.pos = pos_4d.xyz / pos_4d.w;
    data.unresolved_radius = position_data.w;
    data.color = color;
    data.scale = scale;
    data.rotation_quat_xyzw = rotation;
    data.picking_instance_id = picking_instance_id;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    let quad_idx = sphere_quad_index(vertex_idx);

    // Read point data (valid for the entire quad)
    let point_data = read_data(quad_idx);

    let camera_distance = distance(frame.camera_position, point_data.pos);
    let world_scale_factor = average_scale_from_transform(batch.world_from_obj); // TODO(andreas): somewhat costly, should precompute this

    let splat = true; // TODO: use batch.flags

    var out: VertexOut;
    out.color = point_data.color;
    out.point_center = point_data.pos;
    out.picking_instance_id = point_data.picking_instance_id;

    if splat {
        // Gaussian splatting code based on several (sometimes similar) sources:
        // * https://github.com/antimatter15/splat/blob/main/main.js
        // * https://github.com/aras-p/UnityGaussianSplatting/blob/main/package/Shaders/GaussianSplatting.hlsl
        // * https://github.com/BladeTransformerLLC/gauzilla/blob/cef36adf71835eb60d1c6e8b2a2b34af3790c828/src/gsplat.vert
        // * https://github.com/cvlab-epfl/gaussian-splatting-web  // TODO: read this one!
        // * https://github.com/huggingface/gsplat.js/blob/4933ddfecf1a859da473013e63b9d883e298cd15/src/renderers/webgl/programs/RenderProgram.ts
        //
        // TODO: read https://towardsdatascience.com/a-comprehensive-overview-of-gaussian-splatting-e7d570081362
        // How to create your own splats: https://docs.spline.design/e17b7c105ef0433f8c5d2b39d512614e
        //
        // with references to equations in https://www.cs.umd.edu/~zwicker/publications/EWASplatting-TVCG02.pdf
        // and https://repo-sam.inria.fr/fungraph/3d-gaussian-splatting/3d_gaussian_splatting_low.pdf
        //
        // TODO(emilk):
        // * View-dependent color based on spherical-harmonics
        // * Tranparency
        let splat_scale = 1.0; // TODO: let user control it

        let pos2d = frame.projection_from_world * vec4f(out.point_center, 1.0);

        let clip = 1.2 * pos2d.w;
        if (pos2d.z < -clip || pos2d.x < -clip || pos2d.x > clip || pos2d.y < -clip || pos2d.y > clip) {
            // Discard
            out.position = vec4(0.0, 0.0, 0.0, 0.0);
            out.color = vec4f();
            return out;
        }

        // Convert rotation to 3x3 rotation matrix:
        let rot: vec4f = point_data.rotation_quat_xyzw;
        let qx = rot.x;
        let qy = rot.y;
        let qz = rot.z;
        let qw = rot.w;
        let r = mat3x3f(
            1.0 - 2.0 * (qy * qy + qz * qz),
            2.0 * (qx * qy + qw * qz),
            2.0 * (qx * qz - qw * qy),

            2.0 * (qx * qy - qw * qz),
            1.0 - 2.0 * (qx * qx + qz * qz),
            2.0 * (qy * qz + qw * qx),

            2.0 * (qx * qz + qw * qy),
            2.0 * (qy * qz - qw * qx),
            1.0 - 2.0 * (qx * qx + qy * qy),
        );

        // Scale matrix:
        let s = mat3x3f(
            point_data.scale.x, 0.0,                0.0,
            0.0,                point_data.scale.y, 0.0,
            0.0,                0.0,                point_data.scale.z,
        );

        // world-space covariance matrix (called "Vrk" in other sources).
        let cov3d_in_world = r * s * transpose(s) * transpose(r);

        let pos_in_cam: vec3f = frame.view_from_world * vec4f(point_data.pos, 1.0);

        // Project to 2D screen space and clamp:
        let limx: f32 = 1.3 * frame.tan_half_fov.x;
        let limy: f32 = 1.3 * frame.tan_half_fov.y;
        let pos_in_2d = vec3f(
            clamp(pos_in_cam.x / pos_in_cam.z, -limx, limx) * pos_in_cam.z,
            clamp(pos_in_cam.y / pos_in_cam.z, -limy, limy) * pos_in_cam.z,
            pos_in_cam.z,
        );

        // Crate Jacobian for the Taylor approximation of the nonlinear view_from_camera transformation:
        let z_sqr = pos_in_2d.z * pos_in_2d.z;
        let t = pos_in_2d;
        let l = length(t);
        let aspect_ratio = frame.projection_from_view[1][1] / frame.projection_from_view[0][0];
        // EWA Splatting eq.29, with some modifications.
        // I'm not sure how correct this is. The transpose of it also seems to work okish.
        let j: mat3x3f = mat3x3f(
             1.0 / (aspect_ratio * t.z), 0.0,        -t.x / (t.z * t.z),
             0.0,                        1.0 / t.z,  -t.y / (t.z * t.z),
             0.0,                        0.0,         1.0,
        );

        let view3 = mat3x3f(frame.view_from_world.x, frame.view_from_world.y, frame.view_from_world.z);

        // eq.5 in https://repo-sam.inria.fr/fungraph/3d-gaussian-splatting/3d_gaussian_splatting_low.pdf
        let jw: mat3x3f = j * view3;

        // covariance matrix in view space
        let cov2d: mat3x3f = jw * cov3d_in_world * transpose(jw);

        // Find eigen-values of the covariance matrix:
        let mid: f32 = 0.5 * (cov2d[0][0] + cov2d[1][1]);
        let radius: f32 = length(vec2(0.5 * (cov2d[0][0] - cov2d[1][1]), cov2d[0][1]));
        let lambda1: f32 = mid + radius;
        let lambda2: f32 = mid - radius;

        if (lambda2 < 0.0) {
            // Discard
            out.position = vec4(0.0, 0.0, 0.0, 0.0);
            out.color = vec4f();
            return out;
        }

        // Find eigen-vectors of the covariance matrix (the major and minor axes of the ellipsis):
        let diagonal_vector: vec2f = normalize(vec2f(cov2d[0][1], lambda1 - cov2d[0][0]));
        let major_axis: vec2f = min(sqrt(2.0 * lambda1), 1024.0) * diagonal_vector;
        let minor_axis: vec2f = min(sqrt(2.0 * lambda2), 1024.0) * vec2f(diagonal_vector.y, -diagonal_vector.x);

        let local_idx = vertex_idx % 6u;
        let top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.
        let left_right = f32(vertex_idx % 2u) * 2.0 - 1.0; // 1 for a right vertex, -1 for a left vertex.
        let vpos = 2.0 * vec2f(left_right, top_bottom);

        let major: vec2f = (vpos.x * major_axis);
        let minor: vec2f = (vpos.y * minor_axis);

        // Use a correctish Z so we don't need depth-sorting for opaque splats:
        let proj = frame.projection_from_world * vec4f(out.point_center, 1.0);
        let z = proj.z / proj.w;

        out.vpos = vpos;
        out.position = vec4(pos2d.xy / pos2d.w + splat_scale * (major + minor), z, 1.0);
        out.color *= clamp(pos2d.z / pos2d.w + 1.0, 0.0, 1.0);
        out.radius = -666.0; // signal that this is a splat
    } else {
        let world_radius = unresolved_size_to_world(point_data.unresolved_radius, camera_distance,
                                                    frame.auto_size_points, world_scale_factor) +
                        world_size_from_point_size(draw_data.radius_boost_in_ui_points, camera_distance);
        let quad = sphere_or_circle_quad_span(vertex_idx, point_data.pos, world_radius,
                                                has_any_flag(batch.flags, FLAG_DRAW_AS_CIRCLES));

        // Output, transform to projection space and done.
        out.position = apply_depth_offset(frame.projection_from_world * vec4f(quad.pos_in_world, 1.0), batch.depth_offset);
        out.radius = quad.point_resolved_radius;
        out.world_position = quad.pos_in_world;

    }
    return out;
}

// TODO(andreas): move this to sphere_quad.wgsl once https://github.com/gfx-rs/naga/issues/1743 is resolved
// point_cloud.rs has a specific workaround in place so we don't need to split vertex/fragment shader here
//
/// Computes coverage of a 2D sphere placed at `circle_center` in the fragment shader using the currently set camera.
///
/// 2D primitives are always facing the camera - the difference to sphere_quad_coverage is that
/// perspective projection is not taken into account.
fn circle_quad_coverage(world_position: vec3f, radius: f32, circle_center: vec3f) -> f32 {
    let distance = distance(circle_center, world_position);
    let feathering_radius = fwidth(distance) * 0.5;
    return smoothstep(radius + feathering_radius, radius - feathering_radius, distance);
}

fn coverage(world_position: vec3f, radius: f32, point_center: vec3f) -> f32 {
    if is_camera_orthographic() || has_any_flag(batch.flags, FLAG_DRAW_AS_CIRCLES) {
        return circle_quad_coverage(world_position, radius, point_center);
    } else {
        return sphere_quad_coverage(world_position, radius, point_center);
    }
}


@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let splat = in.radius == -666.0;
    if splat {
        let A = -dot(in.vpos, in.vpos);
        if (A < -4.0) {
            discard; // Outside the ellipsis
        }

        var alpha = 1.0;

        // The input color comes with premultiplied alpha.
        // Despite not sorting the splats, this looks okish.
        // (re_renderer has premul alpha-blending turned on,
        // despite not sorting, or having any order-independent transparency).
        var rgba = in.color.rgba;


        if false {
            // In some scenes the alpha seems off, and this seems to help.
            var a = rgba.a;
            rgba /= a; // undo premul
            a = pow(a, 0.1); // change alpha
            rgba *= a; // redo premul
        }

        // TODO(#1611): transparency in rerun
        if false {
            // Fade the gaussian at the edges.
            // This makes the splats way too transparent atm,
            // but I think that is an artifact of use not sorting the splats,
            // but having the Z buffer on, so a transparent edge will occlude other splats.
            // Maybe the splats are too small too?
            // If you turn this on, increase splat_scale to 3-4 or so.
            rgba *= exp(A);
        }

        return rgba;
    }

    let coverage = coverage(in.world_position, in.radius, in.point_center);
    if coverage < 0.001 {
        discard;
    }

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?
    // TODO(andreas): Proper shading
    // TODO(andreas): This doesn't even use the sphere's world position for shading, the world position used here is flat!
    var shading = 1.0;
    if has_any_flag(batch.flags, FLAG_ENABLE_SHADING) {
        shading = max(0.4, sqrt(1.2 - distance(in.point_center, in.world_position) / in.radius)); // quick and dirty coloring
    }
    return vec4f(in.color.rgb * shading, coverage);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    let coverage = coverage(in.world_position, in.radius, in.point_center);
    if coverage <= 0.5 {
        discard;
    }
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    // Output is an integer target, can't use coverage therefore.
    // But we still want to discard fragments where coverage is low.
    // Since the outline extends a bit, a very low cut off tends to look better.
    let coverage = coverage(in.world_position, in.radius, in.point_center);
    if coverage < 1.0 {
        discard;
    }
    return batch.outline_mask;
}
