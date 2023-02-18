// Names chosen to match [`glam`](https://docs.rs/glam/latest/glam/)
type Vec2 = vec2<f32>;
type Vec3 = vec3<f32>;
type Vec4 = vec4<f32>;
type UVec2 = vec2<u32>;
type UVec3 = vec3<u32>;
type UVec4 = vec4<u32>;
type IVec2 = vec2<i32>;
type IVec3 = vec3<i32>;
type IVec4 = vec4<i32>;
type Mat3 = mat4x3<f32>;
type Mat4x3 = mat4x3<f32>;
type Mat4 = mat4x4<f32>;

const f32min = -3.4028235e38;
const f32max = 3.4028235e38;
const f32eps = 0.00000011920928955078125;

const u32min = 0u;
const u32max = 0xFFFFFFFFu;

const X = Vec3(1.0, 0.0, 0.0);
const Y = Vec3(0.0, 1.0, 0.0);
const Z = Vec3(0.0, 0.0, 1.0);

const ZERO = Vec4(0.0, 0.0, 0.0, 0.0);
const ONE  = Vec4(1.0, 1.0, 1.0, 1.0);
