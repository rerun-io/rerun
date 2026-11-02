@group(0) @binding(0)
var source_depth: texture_depth_2d;

@fragment
fn main(@builtin(position) position: vec4f) -> @location(0) f32 {
    return textureLoad(source_depth, vec2i(position.xy), 0);
}
