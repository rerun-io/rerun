#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/encoding.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/srgb.wgsl>

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
const LINESTRIP_TEXTURE_SIZE: i32 = 512;
const POSITION_DATA_TEXTURE_SIZE: i32 = 256;

// Flags
// See lines.rs#LineStripFlags
const CAP_END_TRIANGLE: u32 = 1u;
const CAP_END_ROUND: u32 = 2u;
const CAP_START_TRIANGLE: u32 = 4u;
const CAP_START_ROUND: u32 = 8u;
const NO_COLOR_GRADIENT: u32 = 16u;

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
    active_radius: f32,

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
fn read_strip_data(idx: u32) -> LineStripData {
    // can be u32 once https://github.com/gfx-rs/naga/issues/1997 is solved
    let idx = i32(idx);
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
    pos: Vec3,
    strip_index: u32,
}

// Read and unpack position data at a given location
fn read_position_data(idx: u32) -> PositionData {
    // can be u32 once https://github.com/gfx-rs/naga/issues/1997 is solved
    let idx = i32(idx);
    var raw_data = textureLoad(line_strip_texture, IVec2(idx % LINESTRIP_TEXTURE_SIZE, idx / LINESTRIP_TEXTURE_SIZE), 0);

    var data: PositionData;
    let pos_4d = batch.world_from_obj * Vec4(raw_data.xyz, 1.0);
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

    // Let's assume for starters this vertex is part of a regular quad in a line strip.
    // Fetch position data at the beginning and the end of that quad.
    let pos_data_quad_begin = read_position_data(pos_data_idx);
    let pos_data_quad_end = read_position_data(pos_data_idx + 1u);

    // If the strip indices don't match up for start/end, then we're in a cap triangle!
    let is_cap_triangle = pos_data_quad_begin.strip_index != pos_data_quad_end.strip_index;
    let is_end_cap = is_cap_triangle && local_idx >= 3u;

    // Let's determine which one of the two position data is closer to our vertex.
    // Which tells us things:
    // * what is the closest skeleton position to the current vertex
    // * where do we get the strip data from
    //
    // For caps, we determine the "only valid one" (as one of them belongs to another strip)
    var pos_data_current: PositionData;
    if is_end_cap || (!is_cap_triangle && !is_at_quad_end) {
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
    let strip_radius = unresolved_size_to_world(strip_data.unresolved_radius, length(camera_ray.origin - center_position), frame.auto_size_lines);

    // Active flags are all flags that we react to at the current vertex.
    // I.e. cap flags are only active in the respective cap triangle.
    var currently_active_flags = strip_data.flags & (~(CAP_START_TRIANGLE | CAP_END_TRIANGLE | CAP_START_ROUND | CAP_END_ROUND));

    // Compute quad_dir and correct the currently_active_flags & correct center_position triangle caps.
    var quad_dir: Vec3;
    var is_at_pointy_end = false;
    if is_cap_triangle {
        if is_end_cap && has_any_flag(strip_data.flags, CAP_END_TRIANGLE | CAP_END_ROUND) {
            currently_active_flags |= strip_data.flags & (CAP_END_TRIANGLE | CAP_END_ROUND);
            is_at_pointy_end = is_at_quad_end;
            quad_dir = pos_data_quad_begin.pos - read_position_data(pos_data_idx - 1u).pos; // Go one pos data back
        } else if !is_end_cap && has_any_flag(strip_data.flags, CAP_START_TRIANGLE | CAP_START_ROUND) {
            currently_active_flags |= strip_data.flags & (CAP_START_TRIANGLE | CAP_START_ROUND);
            is_at_pointy_end = !is_at_quad_end;
            quad_dir = read_position_data(pos_data_idx + 2u).pos - pos_data_quad_end.pos; // Go one pos data forward
        } else {
            // Discard vertex.
            center_position = Vec3(0.0/0.0, 0.0/0.0, 0.0/0.0);
        }
        quad_dir = normalize(quad_dir);
    } else {
        quad_dir = normalize(pos_data_quad_end.pos - pos_data_quad_begin.pos);
    }

    var active_radius = strip_radius;
    // If this is a triangle cap, we blow up our ("virtual") quad by twice the size.
    if has_any_flag(currently_active_flags, CAP_START_TRIANGLE | CAP_END_TRIANGLE)  {
        active_radius *= 2.0;
    }

    // Span up the vertex away from the line's axis, orthogonal to the direction to the camera
    let dir_up = normalize(cross(camera_ray.direction, quad_dir));

    var pos: Vec3;
    if is_cap_triangle && is_at_pointy_end {
        // We extend the cap triangle far enough to handle triangle caps,
        // and far enough to do rounded caps without any visible clipping.
        // There is _some_ clipping, but we can't see it ;)
        // If we want to do it properly, we would extend the radius for rounded caps too.
        center_position = pos_data_current.pos + quad_dir * (strip_radius * 4.0 * select(-1.0, 1.0, is_end_cap));
        pos = center_position;
    } else {
        pos = center_position + (active_radius * top_bottom) * dir_up;
    }

    pos -= camera_ray.direction * active_radius;
    center_position -= camera_ray.direction * active_radius;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(pos, 1.0);
    out.position_world = pos;
    out.center_position = center_position;
    out.closest_strip_position = pos_data_current.pos - camera_ray.direction * active_radius;
    out.color = strip_data.color;
    out.active_radius = active_radius;
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
        let signed_distance_to_border = distance_to_skeleton - in.active_radius;
        if signed_distance_to_border > half_pixel_world_size {
            discard;
        }
        coverage = 1.0 - saturate((signed_distance_to_border + half_pixel_world_size) / pixel_world_size);
    }

    // TODO(andreas): lighting setup
    var shading = 1.0;
    if !has_any_flag(in.currently_active_flags, NO_COLOR_GRADIENT) { // TODO(andreas): Flip flag meaning.
        let to_center = in.position_world - in.center_position;
        let relative_distance_to_center_sq = dot(to_center, to_center) / (in.active_radius * in.active_radius);
        shading = max(0.2, 1.0 - relative_distance_to_center_sq) * 0.9;
    }

    return Vec4(in.color.rgb * shading, coverage);
}
