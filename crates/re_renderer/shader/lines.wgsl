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
var line_strip_texture: texture_2d<u32>;

// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
var<private> SEGMENT_TEXTURE_SIZE: i32 = 512;
var<private> LINE_STRIP_TEXTURE_SIZE: i32 = 256;

struct VertexOut {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

struct LineStripData {
    color: vec4<f32>,
    thickness: f32,
    stippling: f32,
}

struct SegmentData {
    pos: vec3<f32>,
    strip_index: i32,
}

// workaround for https://github.com/gfx-rs/naga/issues/2006
fn unpack4x8unorm_workaround(v: u32) -> vec4<f32> {
    var shifted = vec4<u32>(v, v >> u32(8), v >> u32(16), v >> u32(24));
    var bytes = shifted & vec4<u32>(u32(0xFF));
    return vec4<f32>(bytes) * (1.0 / 255.0);
}

fn read_strip_data(strip_index: i32) -> LineStripData {
    var raw_data = textureLoad(line_strip_texture,
        vec2<i32>(strip_index % LINE_STRIP_TEXTURE_SIZE, strip_index / LINE_STRIP_TEXTURE_SIZE), 0).xy;

    var data: LineStripData;
    data.color = unpack4x8unorm_workaround(raw_data.x);
    data.thickness = unpack2x16float(raw_data.y).y;
    data.stippling = f32((raw_data.y >> u32(24)) & u32(0xFF)) * (1.0 / 255.0);
    return data;
}


fn read_segment_data(strip_index: i32) -> SegmentData {
    var raw_data = textureLoad(segment_texture,
        vec2<i32>(i32(strip_index % SEGMENT_TEXTURE_SIZE), strip_index / SEGMENT_TEXTURE_SIZE), 0);

    var data: SegmentData;
    data.pos = raw_data.xyz;
    data.strip_index = bitcast<i32>(raw_data.w);
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    var quad_idx = i32(vertex_idx / u32(6));
    var local_idx = vertex_idx % u32(6);
    var is_start = f32(vertex_idx % u32(2));                   // "left" or "right" on the quad
    var is_top = f32(local_idx <= u32(1) || local_idx == u32(5)); // "top" or "bottom on the quad

    var start = read_segment_data(quad_idx);
    var end = read_segment_data(quad_idx + 1);

    // Is this a degenerated quad?
    if start.strip_index != end.strip_index {
        var out: VertexOut;
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.color = vec4<f32>(0.0);
        return out;
    }

    var next = read_segment_data(quad_idx + 2);
    var strip_data = read_strip_data(start.strip_index);

    var pos = vec3<f32>(0.0);
    pos += select(start.pos, end.pos, is_start > 0.0);
    // TODO: span orthogonal to view vector and line vector
    pos += is_top * vec3<f32>(0.0, strip_data.thickness, 0.0);

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
