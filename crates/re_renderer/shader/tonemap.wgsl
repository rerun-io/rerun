struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) coord: vec2<f32>,
};

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    var c: vec2<f32> = vec2<f32>(-0.79, 0.15);
    var max_iter: u32 = 200u;
    var z: vec2<f32> = (in.coord.xy - vec2<f32>(0.5, 0.5)) * 3.0;

    var i: u32 = 0u;
    loop {
        if (i >= max_iter) {
            break;
        }
        z = vec2<f32>(z.x * z.x - z.y * z.y, z.x * z.y + z.y * z.x) + c;
        if (dot(z, z) > 4.0) {
            break;
        }
        continuing {
            i = i + 1u;
        }
    }

    var t: f32 = f32(i) / f32(max_iter);
    return vec4<f32>(t * 3.0, t * 3.0 - 1.0, t * 3.0 - 2.0, 1.0);
}
