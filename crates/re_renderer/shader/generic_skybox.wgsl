struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    // todo(andreas) implement
    return vec4<f32>(in.texcoord, 0.0, 1.0);
}
