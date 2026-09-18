// Resolve reverse-Z depth to the nearest covered sample, including partially covered pixels.
@group(0) @binding(0)
var source_depth: texture_depth_multisampled_2d;

@fragment
fn main(@builtin(position) position: vec4f) -> @builtin(frag_depth) f32 {
    var nearest_depth = 0.0;
    for (var sample = 0u; sample < textureNumSamples(source_depth); sample += 1u) {
        nearest_depth = max(nearest_depth, textureLoad(source_depth, vec2i(position.xy), i32(sample)));
    }
    return nearest_depth;
}
