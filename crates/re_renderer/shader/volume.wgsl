#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/cube.wgsl>

// ---

struct Ray {
    pos: Vec4,
    dir: Vec4,
    dir_inv: Vec4,
}
fn Ray_init(pos: Vec3, dir: Vec3) -> Ray {
    var ray: Ray;
    ray.pos = Vec4(pos, 1.0);
    ray.dir = Vec4(dir, 0.0);
    ray.dir_inv = 1.0 / ray.dir;

    return ray;
}

struct AABB {
    pmin: Vec3,
    pmax: Vec3,
    center: Vec3,
}
fn AABB_init(pmin: Vec3, pmax: Vec3) -> AABB {
    var aabb: AABB;
    aabb.pmin = pmin;
    aabb.pmax = pmax;
    aabb.center = (pmin + pmax) * 0.5;

    return aabb;
}
fn AABB_ray_intersection(aabb: AABB, ray: Ray) -> f32 {
    let tx1 = (aabb.pmin.x - ray.pos.x) * ray.dir_inv.x;
    let tx2 = (aabb.pmax.x - ray.pos.x) * ray.dir_inv.x;
    let txmin = min(tx1, tx2);
    let txmax = max(tx1, tx2);

    let ty1 = (aabb.pmin.y - ray.pos.y) * ray.dir_inv.y;
    let ty2 = (aabb.pmax.y - ray.pos.y) * ray.dir_inv.y;
    let tymin = min(ty1, ty2);
    let tymax = max(ty1, ty2);

    let tz1 = (aabb.pmin.z - ray.pos.z) * ray.dir_inv.z;
    let tz2 = (aabb.pmax.z - ray.pos.z) * ray.dir_inv.z;
    let tzmin = min(tz1, tz2);
    let tzmax = max(tz1, tz2);

    // tmin < 0 means ray origin is already inside the box
    let tmin = max(0.0, max(txmin, max(tymin, tzmin)));
    let tmax = min(txmax, min(tymax, tzmax));

    if tmax >= tmin { return tmin; } else { return f32max; }
}
/// Result isn't normalized
fn AABB_hit_normal(aabb: AABB, hit: Vec3) -> Vec3 {
    let bias = 1.00001;
    let n = (hit - aabb.center.xyz) / ((aabb.pmax.xyz - aabb.pmin.xyz) * 0.5) * bias;
    // don't floor, flooring negatives is a no-no here
    return Vec3(IVec3(n));
}

// --- Raymarch ---

struct Collision {
    /// The normal at the collision point, in whatever space the original ray was in.
    normal: Vec3,
    t: f32,
    /// The normalized voxel.
    voxel: Vec4,
}
fn Collision_zero() -> Collision {
    return Collision(ZERO.xyz, 0.0, ZERO);
}
fn Collision_max() -> Collision {
    return Collision(ZERO.xyz, f32max, ZERO);
}

/// Returns the absolute fractional part of the difference between v and floor(v).
fn floor_frac(v: f32) -> f32 {
    return (v - floor(v));
}
/// Returns the absolute fractional part of the difference between v and ceil(v).
fn ceil_frac(v: f32) -> f32 {
    return (1.0 - v + floor(v));
}

// TODO: normalized model space only!!
// TODO: mipmaps
fn raymarch_volume(ray: Ray, hit: Vec3) -> Collision {
    var step = sign(ray.dir);
    let delta = abs(ray.dir_inv.xyz);

    let size = volume_info.size;
    // NOTE: shifting to voxel space is just a matter of scale in this case (because
    // the chunk itself is already offset properly), do not treat `hit_ms` as a point!
    var vox_hit = hit * size;
    // NOTE: AFAIK/AFAICT, the only way this can happen is due to floating point
    // imprecision when computing the hit point (ro + rd * t).
    vox_hit = clamp(vox_hit, ZERO.xyz, size);

    var pos = Vec3(floor(vox_hit));
    var pos_prv = pos;

    var tmax = ZERO.xyz;
    if step.x > 0.0 {
        tmax.x = delta.x * ceil_frac(vox_hit.x);
    } else {
        tmax.x = delta.x * floor_frac(vox_hit.x);
    }
    if step.y > 0.0 {
        tmax.y = delta.y * ceil_frac(vox_hit.y);
    } else {
        tmax.y = delta.y * floor_frac(vox_hit.y);
    }
    if step.z > 0.0 {
        tmax.z = delta.z * ceil_frac(vox_hit.z);
    } else {
        tmax.z = delta.z * floor_frac(vox_hit.z);
    }

    let tmax_init = tmax;
    let extents = Vec3(textureDimensions(texture)); // TODO

    let MAX_ITER = 10000u;
    for (var i = 0u; i < MAX_ITER; i = i + 1u) {
        let voxel = textureLoad(texture, IVec3(pos), 0);
        if voxel.a > 0u { // TODO
            let tmax_diff = tmax - tmax_init;
            let t = tmax_diff.x + tmax_diff.y + tmax_diff.z;

            var normal = ZERO.xyz;
            if i > 0u {
                normal = normalize(pos_prv - pos);
            }

            return Collision(normal, t, Vec4(voxel) / 256.0);
        }

        pos_prv = pos;

        let xyz = Vec3(
            f32(tmax.x <= tmax.y && tmax.x < tmax.z),
            f32(tmax.y < tmax.x && tmax.y <= tmax.z),
            f32(tmax.z <= tmax.x && tmax.z < tmax.y),
        );

        pos = pos + (xyz * step.xyz);
        let x_oob = pos.x < 0.0 || pos.x >= extents.x;
        let y_oob = pos.y < 0.0 || pos.y >= extents.y;
        let z_oob = pos.z < 0.0 || pos.z >= extents.z;
        if x_oob || y_oob || z_oob {
            break;
        }

        tmax = tmax + (xyz * delta.xyz);
    }

    return Collision_max();
}

// ---

// TODO: do I have to handle padding here or no?
struct VolumeInfo {
    world_from_model: Mat4,
    model_from_world: Mat4,

    // The actual world-size of the volume.
    size: Vec3,
    // The dimensions (i.e. number of voxels on each axis) of the volume.
    dimensions: UVec3,
};
@group(1) @binding(0)
var<uniform> volume_info: VolumeInfo;

@group(1) @binding(1)
var texture: texture_3d<u32>;

// TODO: will we ever need sampling here?

struct VertexOut {
    @builtin(position) pos_in_clip: Vec4,
    @location(0) pos_in_model: Vec4,
    @location(1) pos_in_world: Vec4,
    @location(2) pos_in_view: Vec4,
    @location(3) normal_in_world: Vec4,
    @location(4) normal_in_view: Vec4,
    @location(5) @interpolate(flat) cam_pos_in_model: Vec4,
};

// TODO: arbitrary model2world transforms + move cam into model space during raytracing

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32) -> VertexOut {
    // Note that since `view_from_world` is an orthonormal matrix, multiplying it from the right
    // means multiplying it with the transpose, meaning multiplying with the inverse!
    // (i.e. we get `world_from_view` for free as long as we only care about directions!)

    let pos_in_model = CUBE_POSITIONS[v_idx];
    let pos_in_world = volume_info.world_from_model * pos_in_model;
    let pos_in_view = pos_in_world.xyz * frame.view_from_world;

    let normal_in_model = Vec4(CUBE_NORMALS[v_idx], 0.0);
    let normal_in_world = volume_info.world_from_model * normal_in_model; // TODO: invese transpose
    let normal_in_view = normal_in_world.xyz * frame.view_from_world;

    let cam_pos_in_model = volume_info.model_from_world * Vec4(frame.camera_position, 1.0);

    var out: VertexOut;
    out.pos_in_clip = frame.projection_from_world * pos_in_world;
    out.pos_in_model = pos_in_model;
    out.pos_in_world = pos_in_world;
    out.pos_in_view = pos_in_view;
    out.normal_in_world = normal_in_world;
    out.normal_in_view = normal_in_view;
    out.cam_pos_in_model = cam_pos_in_model;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    // TODO: prob inverted the name here -.-
    let ray_dir_in_model = normalize(in.pos_in_model.xyz - in.cam_pos_in_model.xyz);

    let ray_in_model = Ray_init(in.cam_pos_in_model.xyz, ray_dir_in_model.xyz);
    let aabb_in_model = AABB_init(CUBE_MIN, CUBE_MAX);
    let t = AABB_ray_intersection(aabb_in_model, ray_in_model);
    if t >= f32max {
         discard;
    }

    let bias = 1.00001;
    let hit_in_model = ray_in_model.pos.xyz + ray_in_model.dir.xyz * t * bias;
    if false {
        //return Vec4(in.pos_in_world.xyz, 1.0);
        //return Vec4(abs(ray_in_model.dir.xyz), 1.0);
        return Vec4(hit_in_model.xyz, 1.0);
    }
    var res = raymarch_volume(ray_in_model, hit_in_model);
    if res.t >= f32max {
        discard;
    }
    if res.t == 0.0 {
        res = Collision(in.normal_in_world.xyz, t, res.voxel);
    }

    // TODO: normally we'd need the raymarch to happen in model space, but since it just so happens
    // that we don't support model transforms on the volume itself right now, this should be
    // straightforward

    let voxel = res.voxel;

    let light_dir = normalize(vec3(1.0, 2.0, 0.0)); // TODO(andreas): proper lighting
    let normal = normalize((volume_info.world_from_model * Vec4(res.normal, 0.0)).xyz);
    let shading = clamp(dot(normal, light_dir), 0.0, 1.0) + 0.2;

    let albedo = clamp(voxel.rgb * (30.0 / res.t), ZERO.rgb, ONE.rgb); // BSAO :D
    let radiance = albedo * shading;

    return Vec4(radiance, 1.0);

    // let r = voxel >> 24u & 0xFFu;
    // let g = voxel >> 16u & 0xFFu;
    // let b = voxel >> 8u & 0xFFu;
    // let a = voxel & 0xFFu;
}
