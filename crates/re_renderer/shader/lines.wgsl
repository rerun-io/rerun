#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/encoding.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/size.wgsl>

@group(1) @binding(0)
var line_strip_texture: texture_2d<f32>;
@group(1) @binding(1)
var position_data_texture: texture_2d<u32>;

struct BatchUniformBuffer {
    world_from_obj: Mat4,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;


// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
let LINESTRIP_TEXTURE_SIZE: i32 = 512;
let POSITION_DATA_TEXTURE_SIZE: i32 = 256;

// Flags
// See lines.wgsl#LineStripFlags
let CAP_END_TRIANGLE: u32 = 1u;
let CAP_END_ROUND: u32 = 2u;
let CAP_START_TRIANGLE: u32 = 4u;
let CAP_START_ROUND: u32 = 8u;
let NO_COLOR_GRADIENT: u32 = 16u;

// A lot of the attributes don't need to be interpolated accross triangles.
// To document that and safe some time we mark them up with @interpolate(flat)
// (see https://www.w3.org/TR/WGSL/#interpolation)
struct VertexOut {
    @builtin(position)
    position: Vec4,

    @location(0) @interpolate(flat)
    color: Vec4,

    @location(1) @interpolate(perspective)
    position_world: Vec3,

    @location(2) @interpolate(perspective)
    center_position: Vec3,

    @location(3) @interpolate(flat)
    radius: f32,

    @location(4) @interpolate(perspective)
    closest_strip_position: Vec3,

    @location(5) @interpolate(flat)
    currently_active_flags: u32,
};

struct LineStripData {
    color: Vec4,
    unresolved_radius: f32,
    stippling: f32,
    flags: u32,
}

// Read and unpack line strip data at a given location
fn read_strip_data(idx: i32) -> LineStripData {
    var raw_data = textureLoad(position_data_texture, IVec2(idx % POSITION_DATA_TEXTURE_SIZE, idx / POSITION_DATA_TEXTURE_SIZE), 0).xy;

    var data: LineStripData;
    data.color = linear_from_srgba(unpack4x8unorm_workaround(raw_data.x));
    // raw_data.y packs { radius: float16, flags: u8, stippling: u8 }
    // See `gpu_data::LineStripInfo` in `lines.rs`
    data.unresolved_radius = unpack2x16float(raw_data.y).y;
    data.flags = ((raw_data.y >> 8u) & 0xFFu);
    data.stippling = f32((raw_data.y >> 16u) & 0xFFu) * (1.0 / 255.0);
    return data;
}

struct PositionData {
    pos: vec3<f32>,
    // i32 for convenience in texture sampling
    // (can be u32 once https://github.com/gfx-rs/naga/issues/1997 is solved)
    strip_index: i32,
}

// Read and unpack position data at a given location
fn read_position_data(idx: i32) -> PositionData {
    var raw_data = textureLoad(line_strip_texture, IVec2(idx % LINESTRIP_TEXTURE_SIZE, idx / LINESTRIP_TEXTURE_SIZE), 0);

    var data: PositionData;
    let pos_4d = batch.world_from_obj * Vec4(raw_data.xyz, 1.0);
    data.pos = pos_4d.xyz / pos_4d.w;
    data.strip_index = bitcast<i32>(raw_data.w);
    return data;
}

fn has_any_flag(flags: u32, flags_to_check: u32) -> bool {
    return (flags & flags_to_check) > 0u;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    //
    // How vertex indices translate to triangles. Example for a single strip with a single quad.
    //
    //   0    1 3    6 7
    //    ____________
    //    \    |\    |\
    //      \  |  \  |  \
    //        \|____\|____\
    //        2 5   4 9   8
    //
    // start cap: 0-3
    // end cap:   7-8
    //

    // Basic properties of the vertex we're at.
    let vertex_idx_with_start_cap_offset = i32(vertex_idx) - 3;
    let is_at_quad_end = (vertex_idx_with_start_cap_offset % 2) == 1;
    let quad_idx = vertex_idx_with_start_cap_offset / 6; // Offset by one triangle (the start cap)
    let local_idx = i32(vertex_idx) % 6;
    let first_tri_or_end_tri = local_idx >= 3; // are we in the first or the second triangle in the current segment.
    let top_bottom = f32(local_idx <= 1 || local_idx == 3) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.

    // Let's assume for starters this vertex is part of a regular quad in a line strip.
    // Fetch position data at the beginning and the end of that quad.
    let pos_data_quad_begin = read_position_data(quad_idx);
    let pos_data_quad_end = read_position_data(quad_idx + 1);

    // If the strip indices don't match up for start/end, then we're in a cap triangle!
    let is_cap_triangle = pos_data_quad_begin.strip_index != pos_data_quad_end.strip_index;

    // Let's determine which one of these is closer to our vertex.
    // Which is two things:
    // * what is the closest skeleton position
    // * where do we get the strip data from
    //
    // For caps, we determine the "only valid one" (as one of them belongs to another strip)
    var pos_data_current: PositionData;
    if (is_cap_triangle && first_tri_or_end_tri) || (!is_cap_triangle && !is_at_quad_end) {
        pos_data_current = pos_data_quad_begin;
    } else {
        pos_data_current = pos_data_quad_end;
    }

    // Note that for caps triangles, the pos_data_current.pos stays constant over the entire triangle!
    // However, to handle things in the fragment shader we need to add a seceond position which is different
    // for start/end of the cap triangle.
    var center_position = pos_data_current.pos;

    // Data valid for the entire strip that this vertex belongs to.
    let strip_data = read_strip_data(pos_data_current.strip_index);

    // Resolve radius.
    // (slight inaccuracy: End caps are going to adjust their center_position)
    let camera_ray = camera_ray_to_world_pos(center_position);
    var radius = unresolved_size_to_world(strip_data.unresolved_radius, length(camera_ray.origin - center_position), 1.0);

    // Active flags are all flags that we react to at the current vertex.
    // I.e. cap flags are only active in the respective cap triangle.
    var currently_active_flags = strip_data.flags & (~(CAP_START_TRIANGLE | CAP_END_TRIANGLE | CAP_START_ROUND | CAP_END_ROUND));

    // Compute quad_dir and correct the flags & correct center position for triangle caps.
    var quad_dir: Vec3;
    if is_cap_triangle {
        var is_at_pointy_end = false;
        if first_tri_or_end_tri && has_any_flag(strip_data.flags, CAP_END_TRIANGLE | CAP_END_ROUND) {
            currently_active_flags |= strip_data.flags & (CAP_END_TRIANGLE | CAP_END_ROUND);
            is_at_pointy_end = is_at_quad_end;
            quad_dir = pos_data_quad_begin.pos - read_position_data(quad_idx - 1).pos; // Go one quad back
        } else if !first_tri_or_end_tri && has_any_flag(strip_data.flags, CAP_START_TRIANGLE | CAP_START_ROUND) {
            currently_active_flags |= strip_data.flags & (CAP_START_TRIANGLE | CAP_START_ROUND);
            is_at_pointy_end = !is_at_quad_end;
            quad_dir = read_position_data(quad_idx + 2).pos - pos_data_quad_end.pos; // Go one quad forward
        } else {
            // Discard vertex.
            center_position = Vec3(0.0/0.0, 0.0/0.0, 0.0/0.0);
        }

        quad_dir = normalize(quad_dir);

        if is_at_pointy_end {
            center_position = pos_data_current.pos + quad_dir * (radius * 4.0 * select(-1.0, 1.0, first_tri_or_end_tri));
            radius = 0.0;
        } else if has_any_flag(currently_active_flags, CAP_START_TRIANGLE | CAP_END_TRIANGLE)  {
            // If this is a triangle cap, we blow up our ("virtual") quad by twice the size.
            // (the pointy end remaings radius==0.0)
            radius *= 2.0;
        }
    } else {
        quad_dir = normalize(pos_data_quad_end.pos - pos_data_quad_begin.pos);
    }

    // Span up the vertex away from the line's axis, orthogonal to the direction to the camera
    let dir_up = normalize(cross(camera_ray.direction, quad_dir));
    let pos = center_position + (radius * top_bottom) * dir_up;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(pos, 1.0);
    out.position_world = pos;
    out.center_position = center_position;
    out.closest_strip_position = pos_data_current.pos;
    out.color = strip_data.color;
    out.radius = radius;
    out.currently_active_flags = currently_active_flags;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {

    var coverage = 1.0;
    if has_any_flag(in.currently_active_flags, CAP_START_ROUND | CAP_END_ROUND) {
        let distance_to_skeleton = length(in.position_world - in.closest_strip_position);
        let pixel_world_size = approx_pixel_world_size_at(length(in.position_world - frame.camera_position));

        // It's important that we do antialias both inwards and outwards of the exact border.
        // If we do only outwards, rectangle outlines won't line up nicely
        let half_pixel_world_size = pixel_world_size * 0.5;
        let signed_distance_to_border = distance_to_skeleton - in.radius;
        if signed_distance_to_border > half_pixel_world_size {
            discard;
        }
        coverage = 1.0 - saturate((signed_distance_to_border + half_pixel_world_size) / pixel_world_size);
    }

    // TODO(andreas): lighting setup
    var shading = 1.0;
    if !has_any_flag(in.currently_active_flags, NO_COLOR_GRADIENT) {
        let to_center = in.position_world - in.center_position;
        let relative_distance_to_center = dot(to_center, to_center) / (in.radius * in.radius);
        shading = max(0.2, 1.0 - relative_distance_to_center) * 0.9;
    }

    return vec4<f32>(in.color.rgb * shading, coverage);
}
