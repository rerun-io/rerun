// As of writing typedefs can't be used for constructing vecN from a single item
// https://github.com/gfx-rs/naga/issues/2105

type Vec2 = vec2<f32>;
type Vec3 = vec3<f32>;
type Vec4 = vec4<f32>;
type UVec2 = vec2<u32>;
type UVec3 = vec3<u32>;
type UVec4 = vec4<u32>;
type IVec2 = vec2<i32>;
type IVec3 = vec3<i32>;
type IVec4 = vec4<i32>;
type Mat4 = mat4x4<f32>;

// Following should be const expressions once available
// https://github.com/gfx-rs/naga/issues/1829

let f32min = -3.4028235e38;
let f32max = 3.4028235e38;
let f32eps = 0.00000011920928955078125;

let u32min = 0u;
let u32max = 0xFFFFFFFFu;

let X = Vec3(1.0, 0.0, 0.0);
let Y = Vec3(0.0, 1.0, 0.0);
let Z = Vec3(0.0, 0.0, 1.0);

let ZERO = Vec3(0.0, 0.0, 0.0);
let ONE  = Vec3(1.0, 1.0, 1.0);
