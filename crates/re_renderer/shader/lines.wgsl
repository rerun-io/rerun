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
var line_strip_texture: texture_2d<f32>;
@group(1) @binding(1)
var position_data_texture: texture_2d<u32>;


// ---------------------------------------------------------------------------
// TODO(andreas): Utilities that belong to a shared header

// workaround for https://github.com/gfx-rs/naga/issues/2006
fn unpack4x8unorm_workaround(v: u32) -> vec4<f32> {
    var shifted = vec4<u32>(v, v >> 8u, v >> 16u, v >> 24u);
    var bytes = shifted & vec4<u32>(0xFFu);
    return vec4<f32>(bytes) * (1.0 / 255.0);
}

// Converts a color from 0-1 sRGB to 0-1 linear
fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = ceil(srgb - 0.04045);
    let under = srgb / 12.92;
    let over = pow((srgb + 0.055) / 1.055,  vec3<f32>(2.4));
    return mix(under, over, cutoff);
}

// Converts a color from 0-1 sRGB to 0-1 linear, leaves alpha untouched
fn linear_from_srgba(srgb_a: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(linear_from_srgb(srgb_a.rgb), srgb_a.a);
}

// ---------------------------------------------------------------------------


// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
var<private> line_strip_texture_SIZE: i32 = 512;
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

// Read and unpack line strip data at a given location
fn read_strip_data(idx: i32) -> LineStripData {
    var raw_data = textureLoad(position_data_texture, vec2<i32>(idx % POSITION_DATA_TEXTURE_SIZE, idx / POSITION_DATA_TEXTURE_SIZE), 0).xy;

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
    var raw_data = textureLoad(line_strip_texture, vec2<i32>(idx % line_strip_texture_SIZE, idx / line_strip_texture_SIZE), 0);

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
    var local_idx = vertex_idx % 6u;
    var top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.

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
    // TODO(andreas): Shading, rounded caps, etc.
    return in.color;
}
