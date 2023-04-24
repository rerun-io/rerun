// Names chosen to match [`glam`](https://docs.rs/glam/latest/glam/)
alias Vec2 = vec2<f32>;
alias Vec3 = vec3<f32>;
alias Vec4 = vec4<f32>;
alias UVec2 = vec2<u32>;
alias UVec3 = vec3<u32>;
alias UVec4 = vec4<u32>;
alias IVec2 = vec2<i32>;
alias IVec3 = vec3<i32>;
alias IVec4 = vec4<i32>;
alias Mat3 = mat3x3<f32>;
alias Mat4x3 = mat4x3<f32>;
alias Mat4 = mat4x4<f32>;

// Extreme values as documented by the spec:
// https://www.w3.org/TR/WGSL/#floating-point-types
const f32max = 0x1.fffffep+127f;  // Largest positive float value.
const f32min = -0x0.fffffep+127f;  // Smallest negative float value.
const f32min_normal = 0x1p-126f;  // Smallest positive normal float value.
// F16 is not implemented yet in Naga https://github.com/gfx-rs/naga/issues/1884
//const f16min = 0x0.ffcp+15h;  // Smallest negative float value.
//const f16max = 0x1.ffcp+15h;  // Largest positive float value.
//const f16min_normal = 0x1p-14h;   // Smallest positive normal float value.
// https://www.w3.org/TR/WGSL/#integer-types
const i32min = -2147483648; // Tint can't handle this being represented as `-0x80000000i`, see https://bugs.chromium.org/p/chromium/issues/detail?id=1439274
const i32max = 0x7fffffffi;
const u32min = 0u;
const u32max = 0xffffffffu;

// Difference between `1.0` and the next larger representable number.
const f32eps = 0.00000011920928955078125;

const X = Vec3(1.0, 0.0, 0.0);
const Y = Vec3(0.0, 1.0, 0.0);
const Z = Vec3(0.0, 0.0, 1.0);

const ZERO = Vec4(0.0, 0.0, 0.0, 0.0);
const ONE  = Vec4(1.0, 1.0, 1.0, 1.0);


// Do NOT use inf() or nan() in your WGSL shaders. Ever.
// The WGSL spec allows implementations to assume that neither Inf or NaN are ever occurring:
// https://www.w3.org/TR/WGSL/#floating-point-evaluation
//
// It will work most of the time, but there are rare cases where this will break.
// (Notably, we had a case where the following commented inf function would silently break shaders when using ANGLE, i.e. in browsers on Windows!)
//
// fn inf() -> f32 {
//     return 1.0 / 0.0;
// }


/// The color to use when we encounter an error.
const ERROR_RGBA = Vec4(1.0, 0.0, 1.0, 1.0);
