#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/encoding.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/depth_offset.wgsl>

@group(1) @binding(0)
var position_texture: texture_2d<f32>;
@group(1) @binding(1)
var strip_data_texture: texture_2d<u32>;
@group(1) @binding(2)
var picking_instance_id_texture: texture_2d<u32>;

struct DrawDataUniformBuffer {
    radius_boost_in_ui_points: f32,
    // In actuality there is way more padding than this since we align all our uniform buffers to
    // 256bytes in order to allow them to be buffer-suballocations.
    // However, wgpu doesn't know this at this point and therefore requires `DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`
    // if we wouldn't add padding here, which isn't available on WebGL.
    _padding: vec4f,
};
@group(1) @binding(3)
var<uniform> draw_data: DrawDataUniformBuffer;

struct BatchUniformBuffer {
    world_from_obj: mat4x4f,
    outline_mask_ids: vec2u,
    picking_layer_object_id: vec2u,
    depth_offset: f32,
    triangle_cap_length_factor: f32,
    triangle_cap_width_factor: f32,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

// Flags
// See lines.rs#LineStripFlags
const FLAG_CAP_END_TRIANGLE: u32 = 1u;
const FLAG_CAP_END_ROUND: u32 = 2u;
const FLAG_CAP_END_EXTEND_OUTWARDS: u32 = 4u;
const FLAG_CAP_START_TRIANGLE: u32 = 8u;
const FLAG_CAP_START_ROUND: u32 = 16u;
const FLAG_CAP_START_EXTEND_OUTWARDS: u32 = 32u;
const FLAG_COLOR_GRADIENT: u32 = 64u;
const FLAG_FORCE_ORTHO_SPANNING: u32 = 128u;

// Special flags used in the fragment shader.
const FLAG_CAP_TRIANGLE: u32 = FLAG_CAP_START_TRIANGLE | FLAG_CAP_END_TRIANGLE;

// A lot of the attributes don't need to be interpolated across triangles.
// To document that and safe some time we mark them up with @interpolate(flat)
// (see https://www.w3.org/TR/WGSL/#interpolation)
struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(flat)
    color: vec4f,

    @location(1) @interpolate(perspective)
    position_world: vec3f,

    @location(2) @interpolate(flat)
    rounded_inner_line_begin: vec3f,

    @location(3) @interpolate(flat)
    rounded_inner_line_end: vec3f,

    @location(4) @interpolate(flat)
    rounded_inner_line_radius: f32,

    @location(5) @interpolate(flat)
    fragment_flags: u32,

    @location(6) @interpolate(flat)
    picking_instance_id: vec2u,
};

struct LineStripData {
    color: vec4f,
    unresolved_radius: f32,
    stippling: f32,
    flags: u32,
    picking_instance_id: vec2u,
}

// Read and unpack line strip data at a given location
fn read_strip_data(idx: u32) -> LineStripData {
    let strip_data_texture_size = textureDimensions(strip_data_texture);
    let raw_data = textureLoad(strip_data_texture,
         vec2u(idx % strip_data_texture_size.x, idx / strip_data_texture_size.x), 0);

    let picking_instance_id_texture_size = textureDimensions(picking_instance_id_texture);
    let picking_instance_id = textureLoad(picking_instance_id_texture,
         vec2u(idx % picking_instance_id_texture_size.x, idx / picking_instance_id_texture_size.x), 0).xy;

    var data: LineStripData;
    data.color = linear_from_srgba(unpack4x8unorm_workaround(raw_data.x));
    // raw_data.y packs { radius: float16, flags: u8, stippling: u8 }
    // See `gpu_data::LineStripInfo` in `lines.rs`
    data.unresolved_radius = unpack2x16float(raw_data.y).y;
    data.flags = ((raw_data.y >> 8u) & 0xFFu);
    data.stippling = f32((raw_data.y >> 16u) & 0xFFu) * (1.0 / 255.0);
    data.picking_instance_id = picking_instance_id;
    return data;
}

struct PositionData {
    pos: vec3f,
    strip_index: u32,
}

// Read and unpack position data at a given location
fn read_position_data(idx: u32) -> PositionData {
    let texture_size = textureDimensions(position_texture);
    let coord = vec2u(idx % texture_size.x, idx / texture_size.x);
    var raw_data = textureLoad(position_texture, coord, 0);

    var data: PositionData;
    let pos_4d = batch.world_from_obj * vec4f(raw_data.xyz, 1.0);
    data.pos = pos_4d.xyz / pos_4d.w;
    data.strip_index = bitcast<u32>(raw_data.w);
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    //
    // How vertex indices translate to triangles. Example for a strip with two quads.
    //
    //  (1)  (0 3 7)  (6 9 13) (12, 16)
    //    ________________________
    //    \      |\      |\      |\
    //      \    |  \    |  \    |  \
    //        \  |    \  |    \  |    \
    //          \|______\|______\|______\
    //        (2 5)  (4 8 11) (10, 14, 15)  (17)
    //
    // (Note winding order flips for every triangle)
    //
    // pos_data_idx:
    //       0       1       2       3      4
    // (position data has a sentinel at the beginning and the end!)
    //
    // start cap:  0..=2
    // quad range: 3..=14
    // end cap:    15..=17
    //

    // Basic properties of the vertex we're at.
    let is_at_quad_end = vertex_idx % 2u == 0u;
    let pos_data_idx = (vertex_idx + 3u) / 6u; // Offset by one triangle (the start cap)
    let local_idx = vertex_idx % 6u;
    let top_bottom = select(-1.0, 1.0, (local_idx <= 1u || local_idx == 3u)); // 1 for a top vertex, -1 for a bottom vertex.

    // There's a left and a right triangle in every quad, see sketch above.
    // Right triangle can form end caps, left triangles can form start caps.
    let is_right_triangle = local_idx >= 3u;

    // Let's assume for starters this vertex is part of a regular quad in a line strip.
    // Fetch position data at the beginning and the end of that quad, as well as the position data before and after this quad.
    let pos_data_quad_before = read_position_data(pos_data_idx - 1u);
    let pos_data_quad_begin = read_position_data(pos_data_idx);
    let pos_data_quad_end = read_position_data(pos_data_idx + 1u);
    let pos_data_quad_after = read_position_data(pos_data_idx + 2u);

    // If the strip indices don't match up for start/end, then we're in a cap triangle!
    let is_cap_triangle = pos_data_quad_begin.strip_index != pos_data_quad_end.strip_index;
    let is_first_quad_after_cap = !is_cap_triangle && (pos_data_quad_begin.strip_index != pos_data_quad_before.strip_index);
    let is_last_quad_before_cap = !is_cap_triangle && (pos_data_quad_end.strip_index != pos_data_quad_after.strip_index);

    // Let's determine which one of the two position data is closer to our vertex.
    // Which tells us things:
    // * what is the closest skeleton position to the current vertex
    // * where do we get the strip data from
    //
    // For caps, we determine the "only valid one" (as one of them belongs to another strip)
    var pos_data_current: PositionData;
    if (is_cap_triangle && is_right_triangle) || (!is_cap_triangle && !is_at_quad_end) {
        pos_data_current = pos_data_quad_begin;
    } else {
        pos_data_current = pos_data_quad_end;
    }

    // The closest "line strip skeleton" position to the current vertex.
    // Various things like end cap or radius boosting can cause adjustments to it.
    var center_position = pos_data_current.pos;

    // Data valid for the entire strip that this vertex belongs to.
    let strip_data = read_strip_data(pos_data_current.strip_index);

    // Compute quad_dir & correct center_position for triangle caps.
    var quad_dir: vec3f;
    var is_at_pointy_arrow_end = false;
    let is_end_cap_triangle = is_cap_triangle && is_right_triangle && has_any_flag(strip_data.flags, FLAG_CAP_END_TRIANGLE);
    let is_start_cap_triangle = is_cap_triangle && !is_right_triangle && has_any_flag(strip_data.flags, FLAG_CAP_START_TRIANGLE);
    if is_end_cap_triangle {
        is_at_pointy_arrow_end = is_at_quad_end;
        quad_dir = pos_data_quad_begin.pos - pos_data_quad_before.pos; // Go one pos data back.
    } else if is_start_cap_triangle {
        is_at_pointy_arrow_end = !is_at_quad_end;
        quad_dir = pos_data_quad_after.pos - pos_data_quad_end.pos; // Go one pos data forward.
    } else if is_cap_triangle {
        // Discard vertex.
        center_position = vec3f(f32max);
    } else {
        quad_dir = pos_data_quad_end.pos - pos_data_quad_begin.pos;
    }
    let quad_length = length(quad_dir);
    quad_dir = quad_dir / quad_length;

    // Resolve radius.
    // (slight inaccuracy: End caps are going to adjust their center_position)
    var camera_ray: Ray;
    if has_any_flag(strip_data.flags, FLAG_FORCE_ORTHO_SPANNING) || is_camera_orthographic() {
        camera_ray = camera_ray_to_world_pos_orthographic(center_position);
    } else {
        camera_ray = camera_ray_to_world_pos_perspective(center_position);
    }
    let camera_distance = distance(camera_ray.origin, center_position);
    let world_scale_factor = average_scale_from_transform(batch.world_from_obj); // TODO(andreas): somewhat costly, should precompute this
    var strip_radius = unresolved_size_to_world(strip_data.unresolved_radius, camera_distance, world_scale_factor);

    // If the triangle cap is longer than the quad would be otherwise, we need to stunt it, otherwise we'd get artifacts.
    var triangle_cap_length = batch.triangle_cap_length_factor * strip_radius;
    let max_triangle_cap_length = quad_length * 0.75; // Having the entire arrow be just triangle head already looks pretty bad, so we're stopping at 75% of the quad length.
    let triangle_cap_size_factor = min(1.0, max_triangle_cap_length / triangle_cap_length);
    triangle_cap_length *= triangle_cap_size_factor;

    // Make space for the end cap if this is either the cap itself or the cap follows right after/before this quad.
    var rounded_inner_line_begin = pos_data_quad_begin.pos;
    if !has_any_flag(strip_data.flags, FLAG_CAP_START_EXTEND_OUTWARDS) && (is_start_cap_triangle || is_first_quad_after_cap) {
        var cap_length =
            f32(has_any_flag(strip_data.flags, FLAG_CAP_START_ROUND)) * strip_radius +
            f32(has_any_flag(strip_data.flags, FLAG_CAP_START_TRIANGLE)) * triangle_cap_length;
        let offset = quad_dir * cap_length;
        rounded_inner_line_begin += offset;
        if !is_at_quad_end || is_start_cap_triangle {
            center_position += offset;
        }
    }
    var rounded_inner_line_end = pos_data_quad_end.pos;
    if !has_any_flag(strip_data.flags, FLAG_CAP_END_EXTEND_OUTWARDS) && (is_end_cap_triangle || is_last_quad_before_cap) {
        var cap_length =
            f32(has_any_flag(strip_data.flags, FLAG_CAP_END_ROUND)) * strip_radius +
            f32(has_any_flag(strip_data.flags, FLAG_CAP_END_TRIANGLE)) * triangle_cap_length;
        let offset = quad_dir * cap_length;
        rounded_inner_line_end -= offset;
        if is_at_quad_end || is_end_cap_triangle {
            center_position -= offset;
        }
    }

    // Boost radius only now that we subtracted/added the cap length.
    // This way we don't get a gap for the outline at the cap.
    if draw_data.radius_boost_in_ui_points > 0.0 {
        let size_boost = world_size_from_point_size(draw_data.radius_boost_in_ui_points, camera_distance);
        strip_radius += size_boost;
        triangle_cap_length += size_boost;
        // Push out positions as well along the quad dir.
        // Otherwise we loose the rounded corners at start/end of a line strip.
        center_position += quad_dir * (size_boost * select(-1.0, 1.0, is_at_quad_end));
    }

    // Filtered list of flags that the fragment shader is interested in.
    var fragment_flags = strip_data.flags & FLAG_COLOR_GRADIENT;

    // If this is a triangle cap, we blow up our ("virtual") quad by a given factor.
    if (is_end_cap_triangle && has_any_flag(strip_data.flags, FLAG_CAP_END_TRIANGLE)) ||
       (is_start_cap_triangle && has_any_flag(strip_data.flags, FLAG_CAP_START_TRIANGLE)) {
        strip_radius *= batch.triangle_cap_width_factor * triangle_cap_size_factor;
        fragment_flags |= FLAG_CAP_TRIANGLE;
    }

    // Span up the vertex away from the line's axis, orthogonal to the direction to the camera
    let dir_up = normalize(cross(camera_ray.direction, quad_dir));
    var pos: vec3f;
    if is_cap_triangle && is_at_pointy_arrow_end {
        // We extend the cap triangle far enough to handle triangle caps.
        center_position += quad_dir * (triangle_cap_length * select(-1.0, 1.0, is_right_triangle));
        pos = center_position;
    } else {
        pos = center_position + (strip_radius * top_bottom * 0.99) * dir_up;
    }

    // Extend the line for rendering smooth joints, as well as round start/end caps.
    let is_at_inner_joint = !is_cap_triangle && !is_first_quad_after_cap && !is_last_quad_before_cap;
    let is_at_quad_with_round_capped_start = !is_at_quad_end && is_first_quad_after_cap && has_any_flag(strip_data.flags, FLAG_CAP_START_ROUND);
    let is_at_quad_with_round_capped_end = is_at_quad_end && is_last_quad_before_cap && has_any_flag(strip_data.flags, FLAG_CAP_END_ROUND);
    if is_at_inner_joint || is_at_quad_with_round_capped_start || is_at_quad_with_round_capped_end {
        let left_right_offset = quad_dir * strip_radius * select(-1.0, 1.0, is_at_quad_end);
        pos += left_right_offset;
    }

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = apply_depth_offset(frame.projection_from_world * vec4f(pos, 1.0), batch.depth_offset);
    out.position_world = pos;
    out.color = strip_data.color;
    out.rounded_inner_line_begin = rounded_inner_line_begin;
    out.rounded_inner_line_end = rounded_inner_line_end;
    out.rounded_inner_line_radius = strip_radius;
    out.fragment_flags = fragment_flags;
    out.picking_instance_id = strip_data.picking_instance_id;

    return out;
}

fn distance_to_line_sq(pos: vec3f, line_a: vec3f, line_b: vec3f) -> f32 {
    let a_to_pos = pos - line_a;
    let a_to_b = line_b - line_a;
    let h = saturate(dot(a_to_pos, a_to_b) / dot(a_to_b, a_to_b));
    let to_line = a_to_pos - a_to_b * h;
    return dot(to_line, to_line);
}

fn distance_to_line(pos: vec3f, line_a: vec3f, line_b: vec3f) -> f32 {
    return sqrt(distance_to_line_sq(pos, line_a, line_b));
}

fn compute_coverage(in: VertexOut) -> f32 {
    var coverage = 1.0;

    if !has_any_flag(in.fragment_flags, FLAG_CAP_TRIANGLE) {
        let distance_to_skeleton = distance_to_line(in.position_world, in.rounded_inner_line_begin, in.rounded_inner_line_end);
        let pixel_world_size = approx_pixel_world_size_at(length(in.position_world - frame.camera_position));

        // It's important that we do antialias both inwards and outwards of the exact border.
        // If we do only outwards, rectangle outlines won't line up nicely
        let half_pixel_world_size = pixel_world_size * 0.5;
        let signed_distance_to_border = distance_to_skeleton - in.rounded_inner_line_radius;
        coverage = 1.0 - saturate((signed_distance_to_border + half_pixel_world_size) / pixel_world_size);
    }

    // Debugging hack: An offset here makes the geometry visible!
    //return coverage + 0.1;
    return coverage;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var coverage = compute_coverage(in);

    if frame.deterministic_rendering == 1 {
        coverage = step(0.5, coverage);
    }

    if coverage < 0.001 {
        discard;
    }

    // TODO(andreas): lighting setup
    var shading = 1.0;
    if has_any_flag(in.fragment_flags, FLAG_COLOR_GRADIENT) {
        let distance_to_inner = distance_to_line_sq(in.position_world, in.rounded_inner_line_begin, in.rounded_inner_line_end);
        shading = max(0.2, 1.0 - distance_to_inner / (in.rounded_inner_line_radius * in.rounded_inner_line_radius)) * 0.9;
    }

    return vec4f(in.color.rgb * shading, coverage);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    var coverage = compute_coverage(in);
    if coverage < 0.5 {
        discard;
    }
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    // Output is an integer target, can't use coverage therefore.
    // But we still want to discard fragments where coverage is too low, otherwise
    // we'd not handle rounded corners etc. correctly.
    // Note that `compute_coverage` may already give coverage values below 1.0 along the
    // "main body of the line", not just the caps.
    var coverage = compute_coverage(in);
    if coverage <= 0.0 {
        discard;
    }
    return batch.outline_mask_ids;
}
