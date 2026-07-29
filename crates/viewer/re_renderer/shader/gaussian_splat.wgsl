// Renderer for 3D gaussian splats.
//
// Uses EWA (Elliptical Weighted Average) splatting: each 3D gaussian is projected to a 2D
// gaussian in screen space by locally linearizing the perspective projection (its Jacobian),
// which maps the 3D covariance ellipsoid to a 2D covariance ellipse. See:
//   - Zwicker et al., "EWA Volume Splatting", 2001: https://www.cs.umd.edu/~zwicker/publications/EWAVolumeSplatting-VIS01.pdf
//   - Kerbl et al., "3D Gaussian Splatting for Real-Time Radiance Field Rendering", 2023: https://repo-sam.inria.fr/fungraph/3d-gaussian-splatting/
//
// Each gaussian is rendered as a screen-space quad spanned in the vertex shader (no vertex
// buffer): the gaussian's 3D covariance (from its per-axis scales and rotation) is projected
// to a 2D covariance in pixel space, and the quad is spanned along its eigenvectors at 3 sigma.
// The fragment shader evaluates the gaussian falloff via the conic (inverse 2D covariance)
// to get per-pixel alpha, output with premultiplied alpha for back-to-front blending.
//
// The vertex/covariance math follows the reference CUDA rasterizer from the Inria 3DGS paper
// above (https://github.com/graphdeco-inria/gaussian-splatting, `cuda_rasterizer/forward.cu`).

#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/quaternion.wgsl>

// Per-gaussian data, one texel per gaussian (see `read_data`).
//
// The scales are split across two textures: the center needs 3 floats, so it goes in an
// `Rgba32Float` texel with the leftover 4th channel carrying `scale_x` for free; the remaining
// two scales then fit in a smaller `Rg32Float` texture rather than wasting another `Rgba` texel.
@group(1) @binding(0)
var position_scale_x_texture: texture_2d<f32>; // xyz center + scale_x
@group(1) @binding(1)
var quat_xyzw_texture: texture_2d<f32>; // rotation, xyzw quaternion
@group(1) @binding(2)
var scale_yz_texture: texture_2d<f32>; // scale_y, scale_z
@group(1) @binding(3)
var color_texture: texture_2d<f32>; // unmultiplied sRGB RGBA (alpha = peak opacity)
@group(1) @binding(4)
var picking_instance_id_texture: texture_2d<u32>;

struct BatchUniformBuffer {
    world_from_obj: mat4x4f,
    flags: u32, // See the `FLAG_*` constants in gaussian_splat.rs.
    // Index of this batch's first gaussian in the shared gaussian-data textures.
    first_gaussian_index: u32,
    _padding: vec2u,
    outline_mask: vec2u,
    picking_layer_object_id: vec2u,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

@group(3) @binding(0)
var gaussian_index_lookup_texture: texture_2d<u32>;

// Must match `FLAG_ENABLE_INDEX_LOOKUP` in gaussian_splat.rs.
const FLAG_ENABLE_INDEX_LOOKUP: u32 = 1u;

// The quad is spanned at this many standard deviations from the center.
// exp(-0.5 * 3.5^2) = 0.0022, i.e. the cut-off contribution is below 1/255 and so invisible after
// 8-bit quantization. (Brush instead sizes each splat to its own 1/255 threshold, which for an
// opaque gaussian works out to ~3.3 sigma; we use a slightly larger fixed value for all opacities.)
// Brush's threshold: `power_threshold = ln(opacity * 255)` in
// https://github.com/ArthurBrussee/brush/blob/3b80985709e2ec04fd6c8622a40e36473647a8e0/crates/brush-render/src/kernels/project_forward.rs#L96
const CUTOFF_SIGMA: f32 = 3.5;

// Minimum per-gaussian alpha (peak opacity * falloff) for a fragment to count as a "hit" in the
// picking & outline passes.
//
// Lower => easier to pick very faint gaussians, but more false positives in empty space.
//
// Real 3DGS scenes are built from many low-opacity gaussians (medians well below 0.1) whose
// accumulation looks solid, so this has to be small or nothing is pickable at all. It still gates
// out empty space and the very faintest wisps, keeping transparent regions hard to pick. Because
// the picking layer is depth-tested and single-sample (no accumulation), this is a per-gaussian
// gate rather than a true "surface where accumulated opacity crosses 0.5".
const PICKING_ALPHA_THRESHOLD: f32 = 0.01;

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(flat)
    color: vec4f, // linear RGBA with unmultiplied/separate alpha (the gaussian's peak opacity)

    // Conic of the projected gaussian: the upper triangle (xx, xy, yy) of the
    // inverse 2D covariance matrix, in pixels.
    @location(1) @interpolate(flat)
    conic: vec3f,

    // Offset of this fragment from the gaussian center, in pixels.
    // All vertices of the quad share the same `w`, so perspective interpolation is linear here.
    @location(2) @interpolate(linear)
    offset_in_pixels: vec2f,

    @location(3) @interpolate(flat)
    picking_instance_id: vec2u,
};

struct GaussianData {
    pos_in_obj: vec3f,
    scale: vec3f,
    quat_xyzw: vec4f,
    color: vec4f,
    picking_instance_id: vec2u,
}

// Read and unpack data at a given location.
fn read_data(idx: u32) -> GaussianData {
    let position_scale_x_texture_size = textureDimensions(position_scale_x_texture);
    let position_scale_x = textureLoad(position_scale_x_texture,
         vec2u(idx % position_scale_x_texture_size.x, idx / position_scale_x_texture_size.x), 0);

    let quat_xyzw_texture_size = textureDimensions(quat_xyzw_texture);
    let quat_xyzw = textureLoad(quat_xyzw_texture,
         vec2u(idx % quat_xyzw_texture_size.x, idx / quat_xyzw_texture_size.x), 0);

    let scale_yz_texture_size = textureDimensions(scale_yz_texture);
    let scale_yz = textureLoad(scale_yz_texture,
         vec2u(idx % scale_yz_texture_size.x, idx / scale_yz_texture_size.x), 0).xy;

    let color_texture_size = textureDimensions(color_texture);
    let color = textureLoad(color_texture,
         vec2u(idx % color_texture_size.x, idx / color_texture_size.x), 0);

    let picking_instance_id_texture_size = textureDimensions(picking_instance_id_texture);
    let picking_instance_id = textureLoad(picking_instance_id_texture,
         vec2u(idx % picking_instance_id_texture_size.x, idx / picking_instance_id_texture_size.x), 0).xy;

    var data: GaussianData;
    data.pos_in_obj = position_scale_x.xyz;
    data.scale = vec3f(position_scale_x.w, scale_yz);
    data.quat_xyzw = quat_xyzw;
    data.color = color;
    data.picking_instance_id = picking_instance_id;
    return data;
}

// A vertex whose quad contributes nothing.
//
// Placed outside the clip volume with a valid `w` (NDC z > 1, beyond the far plane) so it is
// deterministically clipped -- unlike `w = 0`, which is undefined during perspective division.
fn discard_vertex() -> VertexOut {
    var out: VertexOut;
    out.position = vec4f(0.0, 0.0, 2.0, 1.0);
    return out;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    let quad_gaussian_idx = vertex_idx / 6u;
    var gaussian_idx = quad_gaussian_idx;
    if (batch.flags & FLAG_ENABLE_INDEX_LOOKUP) != 0u {
        // Redirect through this view's back-to-front sorted lookup texture (built per-frame on the
        // CPU) so gaussians are drawn far-to-near for correct premultiplied-alpha blending.
        let lookup_idx = quad_gaussian_idx - batch.first_gaussian_index;
        let lookup_texture_size = textureDimensions(gaussian_index_lookup_texture);
        gaussian_idx = batch.first_gaussian_index + textureLoad(
            gaussian_index_lookup_texture,
            vec2u(lookup_idx % lookup_texture_size.x, lookup_idx / lookup_texture_size.x),
            0,
        ).x;
    }
    let data = read_data(gaussian_idx);

    // Two CCW triangles spanning the quad.
    var corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
        vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0),
    );
    let corner = corners[vertex_idx % 6u];

    let world_from_obj = batch.world_from_obj;
    let pos_in_world_4d = world_from_obj * vec4f(data.pos_in_obj, 1.0);
    let pos_in_world = pos_in_world_4d.xyz / pos_in_world_4d.w;

    // --- Project the 3D gaussian to a 2D screen-space gaussian (EWA splatting) ---
    //
    // 1. Build the gaussian's 3D covariance in world space from its scale + rotation.
    // 2. Rotate it into camera space, then locally linearize the perspective projection at the
    //    center (its Jacobian `J`) to get the 2D screen-space covariance `cov2d = J W Σ Wᵀ Jᵀ`.
    //    This linearization is the EWA approximation (exact only at the center).
    // 3. Span the quad along the eigenvectors of `cov2d`; the fragment shader then uses its
    //    inverse (the "conic") to evaluate the per-pixel gaussian falloff.

    // 3D covariance in world space: cov = M * M^T with M = A * R * S,
    // where A is the linear part of world_from_obj, R the rotation, S = diag(scale).
    let rotation = mat3_from_quat(normalize(data.quat_xyzw));
    let world_linear = mat3x3f(world_from_obj[0].xyz, world_from_obj[1].xyz, world_from_obj[2].xyz);
    let rs = mat3x3f(
        rotation[0] * data.scale.x,
        rotation[1] * data.scale.y,
        rotation[2] * data.scale.z,
    );
    let m = world_linear * rs;
    let cov_in_world = m * transpose(m);

    // Camera-space center, with the z axis flipped so that z is positive in front of the camera.
    let pos_in_cam_neg_z = frame.view_from_world * vec4f(pos_in_world, 1.0);
    var t = vec3f(pos_in_cam_neg_z.xy, -pos_in_cam_neg_z.z);
    if t.z <= 0.0 {
        return discard_vertex(); // Behind the camera.
    }

    // View rotation with the same z flip: maps a world direction to a camera direction.
    let view = frame.view_from_world;
    let cam_from_world = mat3x3f(
        vec3f(view[0].xy, -view[0].z),
        vec3f(view[1].xy, -view[1].z),
        vec3f(view[2].xy, -view[2].z),
    );

    let focal = frame.focal_length_in_pixels;

    // Clamp the gaussian center to just outside the frustum before computing the projection
    // Jacobian: it diverges towards the frustum edges, which would blow up the projected
    // covariance of partially-visible gaussians. (Same trick as `forward.cu` in the reference
    // rasterizer linked at the top of this file.)
    let lim = 1.3 * frame.tan_half_fov;
    let txy = clamp(t.xy / t.z, -lim, lim) * t.z;

    // Jacobian of the perspective projection at t (last row unused).
    let jacobian = mat3x3f(
        vec3f(focal.x / t.z, 0.0, 0.0),
        vec3f(0.0, focal.y / t.z, 0.0),
        vec3f(-focal.x * txy.x / (t.z * t.z), -focal.y * txy.y / (t.z * t.z), 0.0),
    );

    // Project: cov2d = J * W * cov3d * W^T * J^T, taking the upper 2x2.
    let jw = jacobian * cam_from_world;
    let cov2d_full = jw * cov_in_world * transpose(jw);
    // Low-pass filter: every gaussian covers at least ~one pixel (anti-aliasing floor).
    let cov2d = vec3f(cov2d_full[0][0] + 0.3, cov2d_full[0][1], cov2d_full[1][1] + 0.3);

    let det = determinant(mat2x2f(cov2d.x, cov2d.y, cov2d.y, cov2d.z));
    if det <= 0.0 {
        return discard_vertex(); // Degenerate covariance.
    }

    // Eigenvalues of the 2D covariance = squared radii along the principal axes.
    let mid = 0.5 * (cov2d.x + cov2d.z);
    let disc = sqrt(max(0.01, mid * mid - det));
    let lambda1 = mid + disc;
    let lambda2 = max(0.01, mid - disc);
    let radius_major_px = CUTOFF_SIGMA * sqrt(lambda1);
    let radius_minor_px = CUTOFF_SIGMA * sqrt(lambda2);

    // Eigenvector of the major axis.
    var axis_major = vec2f(1.0, 0.0);
    if abs(cov2d.y) > 1e-8 {
        axis_major = normalize(vec2f(cov2d.y, lambda1 - cov2d.x));
    } else if cov2d.z > cov2d.x {
        axis_major = vec2f(0.0, 1.0);
    }
    let axis_minor = vec2f(-axis_major.y, axis_major.x);

    let offset_in_pixels = corner.x * radius_major_px * axis_major
                         + corner.y * radius_minor_px * axis_minor;

    // Span the quad around the projected center. All corners keep the center's z & w, so the
    // whole quad depth-tests with the gaussian center's depth.
    let center_clip = frame.projection_from_world * vec4f(pos_in_world, 1.0);
    let offset_ndc = offset_in_pixels * 2.0 / frame.framebuffer_resolution;
    let position = vec4f(center_clip.xy + offset_ndc * center_clip.w, center_clip.zw);

    // Conic: inverse of the 2D covariance, for the gaussian falloff in the fragment shader.
    let conic = vec3f(cov2d.z, -cov2d.y, cov2d.x) / det;

    var out: VertexOut;
    out.position = position;
    out.color = data.color;
    out.conic = conic;
    out.offset_in_pixels = offset_in_pixels;
    out.picking_instance_id = data.picking_instance_id;
    return out;
}

// The gaussian falloff at this fragment, in 0-1.
fn gaussian_falloff(in: VertexOut) -> f32 {
    let d = in.offset_in_pixels;
    let power = -0.5 * (in.conic.x * d.x * d.x + in.conic.z * d.y * d.y) - in.conic.y * d.x * d.y;
    return exp(min(0.0, power));
}

// The gaussian's alpha at this fragment: peak opacity times the gaussian falloff.
fn gaussian_alpha(in: VertexOut) -> f32 {
    // Cap the final alpha at 0.999, matching Brush and the Inria reference rasterizer.
    // (There it keeps the accumulated transmittance non-zero for stable training gradients; we
    // don't train, but keep it for visual parity with those renderers.)
    return min(0.999, in.color.a * gaussian_falloff(in));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let alpha = gaussian_alpha(in);
    // Premultiplied-alpha output, as expected by the `PREMULTIPLIED_ALPHA_BLENDING` blend state.
    return vec4f(in.color.rgb * alpha, alpha);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    if gaussian_alpha(in) < PICKING_ALPHA_THRESHOLD {
        discard;
    }
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    if gaussian_alpha(in) < PICKING_ALPHA_THRESHOLD {
        discard;
    }
    return batch.outline_mask;
}
