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

// TODO: mipmaps
fn raymarch_volume(ray_in_model: Ray, hit_in_model: Vec3) -> Collision {
    var step = sign(ray_in_model.dir);
    let delta = abs(ray_in_model.dir_inv.xyz);

    let dimensions = Vec3(volume_info.dimensions);

    var hit_in_voxel = hit_in_model * dimensions;
    hit_in_voxel = clamp(hit_in_voxel, ZERO.xyz, dimensions);

    if false {
        return Collision(Vec3(0.0, 1.0, 0.0), 1.0, Vec4(ray_in_model.dir.xyz, 1.0));
    }
    if false {
        return Collision(Vec3(0.0, 1.0, 0.0), 1.0, Vec4(hit_in_voxel / dimensions, 1.0));
    }
    if false {
        return Collision(Vec3(0.0, 1.0, 0.0), 1.0, Vec4(hit_in_model, 1.0));
    }

    var pos = Vec3(floor(hit_in_voxel));
    var pos_prv = pos;

    if false {
        return Collision(Vec3(0.0, 1.0, 0.0), 1.0, Vec4(pos / dimensions, 1.0));
    }

    var tmax = ZERO.xyz;
    if step.x > 0.0 {
        tmax.x = delta.x * ceil_frac(hit_in_voxel.x);
    } else {
        tmax.x = delta.x * floor_frac(hit_in_voxel.x);
    }
    if step.y > 0.0 {
        tmax.y = delta.y * ceil_frac(hit_in_voxel.y);
    } else {
        tmax.y = delta.y * floor_frac(hit_in_voxel.y);
    }
    if step.z > 0.0 {
        tmax.z = delta.z * ceil_frac(hit_in_voxel.z);
    } else {
        tmax.z = delta.z * floor_frac(hit_in_voxel.z);
    }

    let tmax_init = tmax;

    let MAX_ITER = 10000u;
    for (var i = 0u; i < MAX_ITER; i = i + 1u) {
        // TODO: we need to do some magic so that we check if the corresponding texcoord in the
        // depth texture contains a Z value that is between the min and max Z values of this voxel.

        // TODO: should there be sampling?????????????

        let cam_npos_in_volume = Vec3(0.5, 0.5, 1.0); // cam at center of front panel

        // TODO: should i floor?
        // let pos_in_model = trunc(pos) / dimensions;
        let pos_in_model = pos / dimensions;
        let v = pos_in_model - cam_npos_in_volume;
        let pos_in_model_backpanel = cam_npos_in_volume + v * distance(cam_npos_in_volume, Vec3(0.1, 0.1, 0.0));


        // TODO: this is wrong! not the correct depth used
        let texcoords_in_volume = Vec3(pos_in_model_backpanel.xy, 0.0); // back panel

        let texcoords = Vec2(pos_in_model_backpanel.x, 1.0 - pos_in_model.y);
        let depth = textureSample(depth_texture, nearest_sampler, texcoords).x;
        let albedo = textureSample(albedo_texture, nearest_sampler, texcoords);

        let npos_in_volume = cam_npos_in_volume + (texcoords_in_volume - cam_npos_in_volume) * depth; //

        // let pos_next = Vec3(pos.x, pos.y, pos.z + 1.0);
        // let pos_in_model_next = pos_next / dimensions;

        let pos_int = IVec3((pos));
        let pos_int_guessed = IVec3(npos_in_volume * (dimensions - 1.0));

//        if pos_in_model_backpanel.x < 0.0 || pos_in_model_backpanel.x > 1.0 {
//            break;
//        }
//        if pos_in_model_backpanel.y < 0.0 || pos_in_model_backpanel.y > 1.0 {
//            break;
//        }
//        if pos_in_model_backpanel.z < 0.0 || pos_in_model_backpanel.z > 1.0 {
//            break;
//        }

        // if pos_in_model.x > 1.0 || pos_in_model.y > 1.0 || pos_in_model.x < 0.0 || pos_in_model.y < 0.0 {
        //     break;
        // }

        if pos_int.z == pos_int_guessed.z {
        // if true {
        // if abs(pos_int.z - pos_int_guessed.z) < 2 {
        // if pos_int.x == pos_int_guessed.x && pos_int.y == pos_int_guessed.y {
        // if pos_in_model.z <= depth && depth <= pos_in_model_next.z {
        // if pos_in_volume.x == pos.x {
            let tmax_diff = tmax - tmax_init;
            let t = tmax_diff.x + tmax_diff.y + tmax_diff.z;
            var normal = ZERO.xyz;
            if i > 0u {
                normal = normalize(pos_prv - pos);
            }

            if false {
                return Collision(normal, t, Vec4(Vec3(min(ONE.xyz, pos_in_model_backpanel)), 1.0));
            }
            if false {
                if pos_in_model_backpanel.x > 1.0 {
                    return Collision(normal, t, Vec4(Vec3(0.0), 1.0));
                }
                if pos_in_model_backpanel.y > 1.0 {
                    return Collision(normal, t, Vec4(Vec3(0.0), 1.0));
                }
                return Collision(normal, t, Vec4(Vec3(pos_in_model_backpanel), 1.0));
            }
            if false {
                return Collision(normal, t, Vec4(Vec3(depth), 1.0));
            }
            if false {
                //return Collision(normal, t, Vec4(Vec3(pos_in_model.z), 1.0));
                // return Collision(normal, t, Vec4(Vec3(pos_in_model_backpanel.xyz), 1.0));
                // return Collision(normal, t, Vec4(Vec3(pos_in_model.z), 1.0));
                // return Collision(normal, t, Vec4(Vec3(abs(pos_int - pos_int_guessed)), 1.0));
            }

            return Collision(normal, t, albedo);
        }

        pos_prv = pos;

        let xyz = Vec3(
            f32(tmax.x <= tmax.y && tmax.x < tmax.z),
            f32(tmax.y < tmax.x && tmax.y <= tmax.z),
            f32(tmax.z <= tmax.x && tmax.z < tmax.y),
        );

        pos = pos + (xyz * step.xyz);
        let x_oob = pos.x < 0.0 || pos.x >= dimensions.x;
        let y_oob = pos.y < 0.0 || pos.y >= dimensions.y;
        let z_oob = pos.z < 0.0 || pos.z >= dimensions.z;
        if x_oob || y_oob || z_oob {
            break;
        }

        tmax = tmax + (xyz * delta.xyz);
    }

    return Collision_max();
}

// ---

struct VolumeInfo {
    world_from_model: Mat4,
    model_from_world: Mat4,
    // The dimensions (i.e. number of voxels on each axis) of the volume.
    dimensions: UVec3,
};
@group(1) @binding(0)
var<uniform> volume_info: VolumeInfo;

@group(1) @binding(1)
var depth_texture: texture_2d<f32>;

@group(1) @binding(2)
var albedo_texture: texture_2d<f32>;

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
    let ray_dir_in_model = normalize(in.pos_in_model.xyz - in.cam_pos_in_model.xyz);

    if false {
        return Vec4(abs(normalize(in.cam_pos_in_model.xyz)), 1.0);
    }
    if false {
        return Vec4(abs(normalize(ray_dir_in_model)), 1.0);
    }

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

    let voxel = res.voxel;
    if false {
        return Vec4(voxel.rgb, 1.0);
        // return Vec4(voxel.rgb, 1.0);
    }

    // let light_dir = normalize(vec3(1.0, 2.0, 0.0)); // TODO(andreas): proper lighting
    // let normal = normalize((volume_info.world_from_model * Vec4(res.normal, 0.0)).xyz);
    let light_dir = Vec3(0.0, 0.0, -1.0);
    let normal = normalize(res.normal);
    let shading = clamp(dot(normal, -light_dir), 0.0, 1.0) + 0.2;

    // let albedo = clamp(voxel.rgb * (30.0 / res.t), ZERO.rgb, ONE.rgb); // BSAO :D
    let albedo = voxel.rgb;
    let radiance = albedo * shading;

    return Vec4(radiance, 1.0);

    // let r = voxel >> 24u & 0xFFu;
    // let g = voxel >> 16u & 0xFFu;
    // let b = voxel >> 8u & 0xFFu;
    // let a = voxel & 0xFFu;
}
