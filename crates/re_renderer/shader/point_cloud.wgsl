#import <./types.wgsl>
#import <./global_bindings.wgsl>

@group(1) @binding(0)
var position_data_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_texture: texture_2d<f32>;


// textureLoad needs i32 right now, so we use that with all sizes & indices to avoid casts
// https://github.com/gfx-rs/naga/issues/1997
var<private> TEXTURE_SIZE: i32 = 1024;

struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) color: Vec4,
    @location(1) world_position: Vec3,
    @location(2) point_center: Vec3,
    @location(3) radius: f32,
};

struct PointData {
    pos: Vec3,
    radius: f32,
    color: Vec4
}

// Read and unpack data at a given location
fn read_data(idx: i32) -> PointData {
    let coord = vec2<i32>(i32(idx % TEXTURE_SIZE), idx / TEXTURE_SIZE);
    let position_data = textureLoad(position_data_texture, coord, 0);
    let color = textureLoad(color_texture, coord, 0);

    var data: PointData;
    data.pos = position_data.xyz;
    data.radius = position_data.w;
    data.color = color;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    // Basic properties of the vertex we're at.
    let quad_idx = i32(vertex_idx) / 6;
    let local_idx = vertex_idx % 6u;
    let top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.
    let left_right = f32(vertex_idx % 2u) * 2.0 - 1.0; // 1 for a right vertex, -1 for a left vertex.

    // Read point data (valid for the entire quad)
    let point_data = read_data(quad_idx);

    // Span quad
    let to_camera = frame.camera_position - point_data.pos;
    let distance_to_camera_sq = dot(to_camera, to_camera);
    let distance_to_camera_inv = inverseSqrt(distance_to_camera_sq); // needed later
    let quad_normal = to_camera * distance_to_camera_inv;
    let quad_right = normalize(cross(quad_normal, frame.view_from_world[1].xyz)); // It's spheres so any orthogonal vector would do.
    let quad_up = cross(quad_right, quad_normal);
    let pos_in_quad = top_bottom * quad_up + left_right * quad_right;

    // But we want to draw pretend-spheres here!
    // If camera gets close to a sphere (or the sphere is large) then outlines of the sphere would not fit on a quad with radius r!
    // Enlarging the quad is one solution, but then Z gets tricky (== we need to write correct Z and not quad Z to depth buffer) since we may get
    // "unnecessary" overlaps. So instead, we change the size _and_ move the sphere closer (using math!)
    let radius_sq = point_data.radius * point_data.radius;
    let camera_offset = radius_sq * distance_to_camera_inv;
    var modified_radius = point_data.radius * distance_to_camera_inv * sqrt(distance_to_camera_sq - radius_sq);
    // We're computing a coverage mask in the fragment shader - make sure the quad doesn't cut off our antialiasing.
    // It's fairly subtle but if we don't do this our spheres look slightly squarish
    modified_radius += frame.pixel_world_size_from_camera_distance / distance_to_camera_inv;
    let pos = point_data.pos + pos_in_quad * modified_radius * 1.0 + camera_offset * quad_normal;
    // normal billboard (spheres are cut off!):
    //      pos = point_data.pos + pos_in_quad * point_data.radius;
    // only enlarged billboard (works but requires z care even for non-overlapping spheres):
    //      modified_radius = length(toCamera) * radius / sqrt(distance_to_camera_sq - radius_sq);
    //      pos = particleCenter + quadPosition * modified_radius;

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(pos, 1.0);
    out.color = point_data.color;
    out.radius = point_data.radius;
    out.world_position = pos;
    out.point_center = point_data.pos;

    return out;
}


// Returns distance to sphere surface (x) and distance to of closest ray hit (y)
// Via https://iquilezles.org/articles/spherefunctions/ but with more verbose names.
fn sphere_distance(ray_origin: Vec3, ray_dir: Vec3, sphere_origin: Vec3, sphere_radius: f32) -> Vec2 {
    let sphere_radius_sq = sphere_radius * sphere_radius;
    let sphere_to_origin = ray_origin - sphere_origin;
    let b = dot(sphere_to_origin, ray_dir);
    let c = dot(sphere_to_origin, sphere_to_origin) - sphere_radius_sq;
    let h = b * b - c;
    let d = sqrt(max(0.0, sphere_radius_sq - h)) - sphere_radius;
    return Vec2(d, -b - sqrt(max(h, 0.0)));
}


@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    let ray_dir = normalize(in.world_position - frame.camera_position);

    // Sphere intersection with anti-aliasing as described by Iq here
    // https://www.shadertoy.com/view/MsSSWV
    // (but rearranged and labled to it's easier to understand!)
    let d = sphere_distance(frame.camera_position, ray_dir, in.point_center, in.radius);
    let smallest_distance_to_sphere = d.x;
    let closest_ray_dist = d.y;
    let pixel_world_size = closest_ray_dist * frame.pixel_world_size_from_camera_distance;
    if  smallest_distance_to_sphere > pixel_world_size {
        discard;
    }
    let coverage = 1.0 - clamp(smallest_distance_to_sphere / pixel_world_size, 0.0, 1.0);

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?

    // TODO(andreas): Proper shading
    let shading = max(0.2, 1.2 - distance(in.point_center, in.world_position) / in.radius); // quick and dirty coloring)
    return vec4(in.color.rgb * shading, coverage);
}
