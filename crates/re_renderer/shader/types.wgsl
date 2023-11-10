// Extreme values as documented by the spec:
// https://www.w3.org/TR/WGSL/#floating-point-types
//const f32max = 0x1.fffffep+127f;  // Largest positive float value.
const f32max = 3.40282e38;  // ⚠️ Should be above value, using a smaller one to work around https://github.com/gfx-rs/naga/issues/2436.
const f32min = -0x0.fffffep+127f;  // Smallest negative float value.
const f32min_normal = 0x1p-126f;  // Smallest positive normal float value.
// F16 is not implemented yet in Naga https://github.com/gfx-rs/naga/issues/1884
//const f16min = 0x0.ffcp+15h;  // Smallest negative float value.
//const f16max = 0x1.ffcp+15h;  // Largest positive float value.
//const f16min_normal = 0x1p-14h;   // Smallest positive normal float value.
// https://www.w3.org/TR/WGSL/#integer-types
const i32min = -2147483648; // Naga has some issues with correct negative hexadecimal numbers https://github.com/gfx-rs/naga/issues/2314
const i32max = 0x7fffffffi;
const u32min = 0u;
const u32max = 0xffffffffu;

// Difference between `1.0` and the next larger representable number.
const f32eps = 0.00000011920928955078125;

const X = vec3f(1.0, 0.0, 0.0);
const Y = vec3f(0.0, 1.0, 0.0);
const Z = vec3f(0.0, 0.0, 1.0);

const ZERO = vec4f(0.0, 0.0, 0.0, 0.0);
const ONE  = vec4f(1.0, 1.0, 1.0, 1.0);


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
const ERROR_RGBA = vec4f(1.0, 0.0, 1.0, 1.0);
