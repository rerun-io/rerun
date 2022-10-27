#import <./global_bindings.wgsl>

@group(1) @binding(0)
var position_data_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_texture: texture_2d<f32>;


// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
var<private> TEXTURE_SIZE: i32 = 1024;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) point_center: vec3<f32>,
    @location(3) radius: f32,
};

struct PointData {
    pos: vec3<f32>,
    radius: f32,
    color: vec4<f32>
}

// Read and unpack data at a given location
fn read_data(idx: i32) -> PointData {
    let coord = vec2<i32>(i32(idx % TEXTURE_SIZE), idx / TEXTURE_SIZE);
    var position_data = textureLoad(position_data_texture, coord, 0);
    var color = textureLoad(color_texture, coord, 0);

    var data: PointData;
    data.pos = position_data.xyz;
    data.radius = position_data.w;
    data.color = color;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    // Basic properties of the vertex we're at.
    var quad_idx = i32(vertex_idx) / 6;
    var local_idx = vertex_idx % 6u;
    var top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.
    var left_right = f32(vertex_idx % 2u) * 2.0 - 1.0; // 1 for a right vertex, -1 for a left vertex.

    // Read point data (valid for the entire quad)
    var point_data = read_data(quad_idx);

    // Span quad
    var to_camera = frame.camera_position - point_data.pos;
    var distance_to_camera_sq = dot(to_camera, to_camera);
    var distance_to_camera_inv = inverseSqrt(distance_to_camera_sq); // needed later
    var quad_normal = to_camera * distance_to_camera_inv;
    var quad_right = normalize(cross(quad_normal, frame.view_from_world[1].xyz)); // It's spheres so any orthogonal vector would do.
    var quad_up = cross(quad_right, quad_normal);
    var pos_in_quad = top_bottom * quad_up + left_right * quad_right;

    // But we want to draw pretend-spheres here!
    // If camera gets close to a sphere (or the sphere is large) then outlines of the sphere would not fit on a quad with radius r!
    // Enlarging the quad is one solution, but then Z gets tricky (== we need to write correct Z and not quad Z to depth buffer) since we may get
    // "unnecessary" overlaps. So instead, we change the size _and_ move the sphere closer (using math!)
    let radius_sq = point_data.radius * point_data.radius;
    let camera_offset = point_data.radius * point_data.radius * distance_to_camera_inv;
    let modified_radius = point_data.radius * distance_to_camera_inv * sqrt(distance_to_camera_sq - radius_sq);
    var pos = point_data.pos + pos_in_quad * modified_radius + camera_offset * quad_normal;
    // normal billboard (spheres are cut off!):
    //      pos = point_data.pos + pos_in_quad * point_data.radius;
    // only enlarged billboard (works but requires z care even for non-overlapping spheres):
    //      modified_radius = length(toCamera) * radius / sqrt(distance_to_camera_sq - radius_sq);
    //      pos = particleCenter + quadPosition * modified_radius;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * vec4<f32>(pos, 1.0);
    out.color = point_data.color;
    out.radius = point_data.radius;
    out.world_position = pos;
    out.point_center = point_data.pos;

    return out;
}

// Return how far the closest intersection point is from ray_origin.
// Returns -1.0 if no intersection happend
fn sphere_intersect(sphere_pos: vec3<f32>, radius_sq: f32, ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> f32 {
    let sphere_to_origin = ray_origin - sphere_pos;
    let b = dot(sphere_to_origin, ray_dir);
    let c = dot(sphere_to_origin, sphere_to_origin) - radius_sq;
    let discriminant = b * b - c;
    if (discriminant < 0.0) {
        return -1.0;
    }
    return -b - sqrt(discriminant);
}


@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // TODO(andreas): Pass around squared radius instead.
    let ray_dir = normalize(in.world_position - frame.camera_position);
    if sphere_intersect(in.point_center, in.radius * in.radius, frame.camera_position, ray_dir) < 0.0 {
        discard;
    }

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?

    // TODO(andreas): Proper shading
    let shading = min(1.0, 1.2 - distance(in.point_center, in.world_position) / in.radius); // quick and dirty coloring)
    return in.color * shading;
}
