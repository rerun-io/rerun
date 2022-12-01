#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/encoding.wgsl>
#import <./utils/camera.wgsl>

@group(1) @binding(0)
var line_strip_texture: texture_2d<f32>;
@group(1) @binding(1)
var position_data_texture: texture_2d<u32>;

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

struct VertexOut {
    @builtin(position)
    position: Vec4,

    @location(0) @interpolate(flat)
    color: Vec4,

    @location(1) @interpolate(perspective)
    position_world: Vec3,

    @location(2) @interpolate(perspective)
    closest_line_skeleton_position: Vec3,

    @location(3) @interpolate(flat)
    line_radius: f32,

    @location(4) @interpolate(flat)
    round_cap: u32,
};

struct LineStripData {
    color: Vec4,
    radius: f32,
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
    data.radius = unpack2x16float(raw_data.y).y;
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
    data.pos = raw_data.xyz;
    data.strip_index = bitcast<i32>(raw_data.w);
    return data;
}

fn has_any_flag(flags: u32, flags_to_check: u32) -> bool {
    return (flags & flags_to_check) > 0u;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    // Basic properties of the vertex we're at.
    let is_at_quad_end = (i32(vertex_idx) % 2) == 1;
    let quad_idx = i32(vertex_idx) / 6;
    let local_idx = vertex_idx % 6u;
    let top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.

    // Position data at the beginning and the end of the current quad.
    let pos_data_quad_begin = read_position_data(quad_idx);
    let pos_data_quad_end = read_position_data(quad_idx + 1);

    // Position data at and before the vertex
    var pos_data_current: PositionData;
    if is_at_quad_end {
        pos_data_current = pos_data_quad_end;
    } else {
        pos_data_current = pos_data_quad_begin;
    }

    // True if this is a trailing end quad - should either turn into a cap for the previous strip or collapse!
    let is_trailing_quad = pos_data_quad_begin.strip_index != pos_data_quad_end.strip_index;

    // Data valid for the entire strip that this vertex belongs to.
    var strip_data = read_strip_data(pos_data_quad_begin.strip_index);

    // Check if we should generate an end cap
    if is_trailing_quad {

    }

    // Calculate the direction the current quad is facing in.
    var quad_dir: Vec3;
    var center_position = pos_data_current.pos; // line center of the quad (triangle for caps) we're spanning.
    var closest_line_skeleton_position = pos_data_current.pos;
    var round_cap = 0u;
    if is_trailing_quad {
        if has_any_flag(strip_data.flags, CAP_END_TRIANGLE | CAP_END_ROUND) && local_idx < 3u {
            quad_dir = pos_data_quad_begin.pos - read_position_data(quad_idx - 1).pos;
            quad_dir = normalize(quad_dir);

            round_cap = u32(has_any_flag(strip_data.flags, CAP_END_ROUND));

            var size_factor = 1.0 + f32(has_any_flag(strip_data.flags, CAP_END_TRIANGLE));

            if is_at_quad_end {
                // The pointy end.
                closest_line_skeleton_position = pos_data_quad_begin.pos;
                center_position = pos_data_quad_begin.pos +
                                       quad_dir * (strip_data.radius * 4.0);
                strip_data.radius = 0.0;
            } else if round_cap == 0u {
                // Thick start of the triangle cap.
                strip_data.radius *= 2.0;
            }
        } else {
            quad_dir = ZERO;
        }
    } else {
        quad_dir = pos_data_quad_begin.pos - pos_data_quad_end.pos;
        quad_dir = normalize(quad_dir);
    }

    // Span up the vertex away from the line's axis, orthogonal to the direction to the camera
    let to_camera = camera_ray_to_world_pos(center_position).direction;
    let dir_up = normalize(cross(to_camera, quad_dir));
    let pos = center_position + (strip_data.radius * top_bottom) * dir_up;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(pos, 1.0);
    out.position_world = pos;
    out.closest_line_skeleton_position = closest_line_skeleton_position;
    out.color = strip_data.color;
    out.line_radius = strip_data.radius;
    out.round_cap = round_cap;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    let distance_to_skeleton = length(in.position_world - in.closest_line_skeleton_position);
    let relative_distance_to_skeleton = distance_to_skeleton / in.line_radius;

    var coverage = 1.0;
    if in.round_cap != 0u {
        if relative_distance_to_skeleton > 1.0 {
            discard;
        }
    }

    // TODO(andreas): proper shading/lighting, etc.
    let shading = max(0.2, 1.2 - relative_distance_to_skeleton);
    return in.color * shading;
}
