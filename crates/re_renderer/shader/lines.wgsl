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

struct VertexOut {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

struct SegmentData {
    start: vec3<f32>,
    end: vec3<f32>,
    color: vec3<f32>,
}

fn read_segment_data(quad_idx: i32) -> SegmentData {
    // textureDimensions currently returns i32 https://github.com/gfx-rs/naga/issues/1985
    var instance_texture_width = textureDimensions(segment_texture, 0).x;
    // Need to pass i32 https://github.com/gfx-rs/naga/issues/1997
    var line_info_start = textureLoad(segment_texture,
        vec2<i32>(i32(quad_idx % instance_texture_width), quad_idx / instance_texture_width), 0);

    var next_quad_idx = quad_idx + 1;
    var line_info_end = textureLoad(segment_texture,
        vec2<i32>(i32(next_quad_idx % instance_texture_width), next_quad_idx / instance_texture_width), 0);

    var data: SegmentData;
    data.start = line_info_start.xyz;
    data.color = unpack4x8unorm(bitcast<u32>(line_info_start.w)).rgb;
    data.end = line_info_end.xyz;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    var quad_idx = i32(vertex_idx / u32(6));
    var is_start = f32(vertex_idx % u32(2));                   // "left" or "right" on the quad
    var is_top = f32(vertex_idx <= u32(1) || vertex_idx == u32(5)); // "top" or "bottom on the quad

    var segment = read_segment_data(quad_idx);

    var pos = vec3<f32>(0.0);
    pos += select(segment.end, segment.start, is_start > 0.0);
    // TODO: span orthogonal to view vector and line vector
    pos += is_top * vec3<f32>(0.0, 1.0, 0.0);

    var out: VertexOut;
    out.position = frame.projection_from_world * vec4<f32>(pos, 1.0);
    out.color = vec4<f32>(segment.color, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // TODO(andreas): Rounded caps
    return in.color;
}
