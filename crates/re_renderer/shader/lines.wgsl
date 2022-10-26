#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/encoding.wgsl>

@group(1) @binding(0)
var line_strip_texture: texture_2d<f32>;
@group(1) @binding(1)
var position_data_texture: texture_2d<u32>;

// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
let LINESTRIP_TEXTURE_SIZE: i32 = 512;
let POSITION_DATA_TEXTURE_SIZE: i32 = 256;

struct VertexOut {
    @location(0) color: Vec4,
    @builtin(position) position: Vec4,
};

struct LineStripData {
    color: Vec4,
    thickness: f32,
    stippling: f32,
}

// Read and unpack line strip data at a given location
fn read_strip_data(idx: i32) -> LineStripData {
    var raw_data = textureLoad(position_data_texture, IVec2(idx % POSITION_DATA_TEXTURE_SIZE, idx / POSITION_DATA_TEXTURE_SIZE), 0).xy;

    var data: LineStripData;
    data.color = linear_from_srgba(unpack4x8unorm_workaround(raw_data.x));
    // raw_data.y packs { thickness: float16, unused: u8, stippling: u8 }
    // See `gpu_data::LineStripInfo` in `lines.rs`
    data.thickness = unpack2x16float(raw_data.y).y;
    data.stippling = f32((raw_data.y >> 24u) & 0xFFu) * (1.0 / 255.0);
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

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    // Basic properties of the vertex we're at.
    let is_at_quad_end = i32(vertex_idx) % 2;
    let quad_idx = i32(vertex_idx) / 6;
    let local_idx = vertex_idx % 6u;
    let top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.

    // data at and before the vertex
    let pos_data_idx = quad_idx + is_at_quad_end;
    let pos_data_before = read_position_data(pos_data_idx - 1);
    var pos_data_current = read_position_data(pos_data_idx);
    let pos_data_next = read_position_data(pos_data_idx + 1);

    // Are we at the end of a previous and start of a new line strip? If so, collapse the quad between them.
    if is_at_quad_end == 1 && pos_data_before.strip_index != pos_data_current.strip_index {
        pos_data_current = pos_data_before;
    }

    // Data valid for the entire strip
    var strip_data = read_strip_data(pos_data_current.strip_index);

    // Calculate the direction the current quad is facing in.
    var quad_dir = pos_data_current.pos - pos_data_before.pos;
    if is_at_quad_end == 0 {
        quad_dir = pos_data_next.pos - pos_data_current.pos;
    }
    quad_dir = normalize(quad_dir);

    // Span up the vertex away from the line's axis, orthogonal to the direction to the camera
    let to_camera = normalize(frame.camera_position - pos_data_current.pos);
    let dir_up = normalize(cross(to_camera, quad_dir));
    let pos = pos_data_current.pos + (strip_data.thickness * top_bottom) * dir_up;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(pos, 1.0);
    out.color = strip_data.color;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    // TODO(andreas): Shading, rounded caps, etc.
    return in.color;
}
