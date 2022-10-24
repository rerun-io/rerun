struct FrameUniformBuffer {
    view_from_world: mat4x3<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,

    camera_position: vec3<f32>,
    top_right_screen_corner_in_view: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

@group(1) @binding(0)
var segment_texture: texture_2d<f32>;
@group(1) @binding(1)
var position_data_texture: texture_2d<u32>;

// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
var<private> SEGMENT_TEXTURE_SIZE: i32 = 512;
var<private> POSITION_DATA_TEXTURE_SIZE: i32 = 256;

struct VertexOut {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

struct LineStripData {
    color: vec4<f32>,
    thickness: f32,
    stippling: f32,
}

struct PositionData {
    pos: vec3<f32>,
    strip_index: i32,
}

// workaround for https://github.com/gfx-rs/naga/issues/2006
fn unpack4x8unorm_workaround(v: u32) -> vec4<f32> {
    var shifted = vec4<u32>(v, v >> u32(8), v >> u32(16), v >> u32(24));
    var bytes = shifted & vec4<u32>(u32(0xFF));
    return vec4<f32>(bytes) * (1.0 / 255.0);
}

// Converts a color from 0-1 sRGB to 0-1 linear
// adapted from https://gamedev.stackexchange.com/a/148088
fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = ceil(srgb - 0.04045);
    let higher = pow((srgb + 0.055) / 1.055,  vec3<f32>(2.4));
    let lower = srgb / 12.92;

    return mix(lower, higher, cutoff);
}

fn linear_from_srgba(srgb_a: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(linear_from_srgb(srgb_a.rgb), srgb_a.a);
}

fn read_strip_data(strip_index: i32) -> LineStripData {
    var raw_data = textureLoad(position_data_texture,
        vec2<i32>(strip_index % POSITION_DATA_TEXTURE_SIZE, strip_index / POSITION_DATA_TEXTURE_SIZE), 0).xy;

    var data: LineStripData;
    data.color = linear_from_srgba(unpack4x8unorm_workaround(raw_data.x));
    data.thickness = unpack2x16float(raw_data.y).y;
    data.stippling = f32((raw_data.y >> u32(24)) & u32(0xFF)) * (1.0 / 255.0);
    return data;
}


fn read_position_data(segment_idx: i32) -> PositionData {
    // Negative indices are defined to return all zero!
    var raw_data = textureLoad(segment_texture,
        vec2<i32>(i32(segment_idx % SEGMENT_TEXTURE_SIZE), segment_idx / SEGMENT_TEXTURE_SIZE), 0);

    var data: PositionData;
    data.pos = raw_data.xyz;
    data.strip_index = bitcast<i32>(raw_data.w);
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    // Basic properties of the vertex we're at.
    var is_at_quad_end = i32(vertex_idx) % 2;
    var quad_idx = i32(vertex_idx) / 6;
    var local_idx = vertex_idx % u32(6);
    var top_bottom = f32(local_idx <= u32(1) || local_idx == u32(5)) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.

    // data at and before the vertex
    var pos_data_idx = quad_idx + is_at_quad_end;
    var pos_data_before = read_position_data(pos_data_idx - 1);
    var pos_data_current = read_position_data(pos_data_idx);
    var pos_data_next = read_position_data(pos_data_idx + 1);

    // Is this a degenerated quad? Collapse it!
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
    var to_camera = normalize(frame.camera_position - pos_data_current.pos);
    var dir_up = normalize(cross(to_camera, quad_dir));
    var pos = pos_data_current.pos + (strip_data.thickness * top_bottom) * dir_up;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * vec4<f32>(pos, 1.0);
    out.color = strip_data.color;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // TODO(andreas): Rounded caps
    return in.color;
}
