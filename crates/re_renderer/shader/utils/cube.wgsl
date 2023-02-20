#import <../types.wgsl>

const vmin: f32 = 0.0;
const vmax: f32 = 1.0;

const CUBE_MIN: Vec3 = Vec3(vmin, vmin, vmin);
const CUBE_MAX: Vec3 = Vec3(vmax, vmax, vmax);

var<private> CUBE_POSITIONS: array<Vec4, 36> = array<Vec4, 36>(
    // Front
    Vec4(vmin, vmin, vmax, 1.0),
    Vec4(vmax, vmin, vmax, 1.0),
    Vec4(vmax, vmax, vmax, 1.0),
    Vec4(vmax, vmax, vmax, 1.0),
    Vec4(vmin, vmax, vmax, 1.0),
    Vec4(vmin, vmin, vmax, 1.0),
    // Back
    Vec4(vmin, vmax, vmin, 1.0),
    Vec4(vmax, vmax, vmin, 1.0),
    Vec4(vmax, vmin, vmin, 1.0),
    Vec4(vmax, vmin, vmin, 1.0),
    Vec4(vmin, vmin, vmin, 1.0),
    Vec4(vmin, vmax, vmin, 1.0),
    // Right
    Vec4(vmax, vmin, vmin, 1.0),
    Vec4(vmax, vmax, vmin, 1.0),
    Vec4(vmax, vmax, vmax, 1.0),
    Vec4(vmax, vmax, vmax, 1.0),
    Vec4(vmax, vmin, vmax, 1.0),
    Vec4(vmax, vmin, vmin, 1.0),
    // Left
    Vec4(vmin, vmin, vmax, 1.0),
    Vec4(vmin, vmax, vmax, 1.0),
    Vec4(vmin, vmax, vmin, 1.0),
    Vec4(vmin, vmax, vmin, 1.0),
    Vec4(vmin, vmin, vmin, 1.0),
    Vec4(vmin, vmin, vmax, 1.0),
    // Top
    Vec4(vmax, vmax, vmin, 1.0),
    Vec4(vmin, vmax, vmin, 1.0),
    Vec4(vmin, vmax, vmax, 1.0),
    Vec4(vmin, vmax, vmax, 1.0),
    Vec4(vmax, vmax, vmax, 1.0),
    Vec4(vmax, vmax, vmin, 1.0),
    // Bottom
    Vec4(vmax, vmin, vmax, 1.0),
    Vec4(vmin, vmin, vmax, 1.0),
    Vec4(vmin, vmin, vmin, 1.0),
    Vec4(vmin, vmin, vmin, 1.0),
    Vec4(vmax, vmin, vmin, 1.0),
    Vec4(vmax, vmin, vmax, 1.0),
);

var<private> CUBE_NORMALS: array<Vec3, 36> = array<Vec3, 36>(
    // Front
    Vec3(0.0, 0.0, 1.0),
    Vec3(0.0, 0.0, 1.0),
    Vec3(0.0, 0.0, 1.0),
    Vec3(0.0, 0.0, 1.0),
    Vec3(0.0, 0.0, 1.0),
    Vec3(0.0, 0.0, 1.0),
    // Back
    Vec3(0.0, 0.0, -1.0),
    Vec3(0.0, 0.0, -1.0),
    Vec3(0.0, 0.0, -1.0),
    Vec3(0.0, 0.0, -1.0),
    Vec3(0.0, 0.0, -1.0),
    Vec3(0.0, 0.0, -1.0),
    // Right
    Vec3(1.0, 0.0, 0.0),
    Vec3(1.0, 0.0, 0.0),
    Vec3(1.0, 0.0, 0.0),
    Vec3(1.0, 0.0, 0.0),
    Vec3(1.0, 0.0, 0.0),
    Vec3(1.0, 0.0, 0.0),
    // Left
    Vec3(-1.0, 0.0, 0.0),
    Vec3(-1.0, 0.0, 0.0),
    Vec3(-1.0, 0.0, 0.0),
    Vec3(-1.0, 0.0, 0.0),
    Vec3(-1.0, 0.0, 0.0),
    Vec3(-1.0, 0.0, 0.0),
    // Top
    Vec3(0.0, 1.0, 0.0),
    Vec3(0.0, 1.0, 0.0),
    Vec3(0.0, 1.0, 0.0),
    Vec3(0.0, 1.0, 0.0),
    Vec3(0.0, 1.0, 0.0),
    Vec3(0.0, 1.0, 0.0),
    // Bottom
    Vec3(0.0, -1.0, 0.0),
    Vec3(0.0, -1.0, 0.0),
    Vec3(0.0, -1.0, 0.0),
    Vec3(0.0, -1.0, 0.0),
    Vec3(0.0, -1.0, 0.0),
    Vec3(0.0, -1.0, 0.0),
);
