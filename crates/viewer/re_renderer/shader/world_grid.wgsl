#import <./global_bindings.wgsl>
#import <./utils/interpolation.wgsl>
#import <./utils/plane.wgsl>
struct WorldGridUniformBuffer {
    color: vec4f,

    /// Plane equation, normal + distance.
    plane: Plane,

    /// How far apart the closest sets of lines are.
    spacing: f32,

    /// How thick the lines are in UI units.
    thickness_ui: f32,

    /// Offset of the grid along its normal.
    normal_offset: f32,
}

@group(1) @binding(0)
var<uniform> config: WorldGridUniformBuffer;

struct VertexOutput {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(flat) // Result doesn't differ per vertex.
    next_cardinality_interpolation: f32,
};

// Spans a large quad where centered around the camera.
//
// This gives us the "canvas" to drawn the grid on.
// Compared to a fullscreen pass, we get the z value (and thus early z testing) for free,
// as well as never covering the screen above the horizon.
@vertex
fn main_vs(@builtin(vertex_index) v_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let camera_plane_distance_world = abs(distance_to_plane(config.plane, frame.camera_position));

    // Scale the plane geometry based on the distance to the camera.
    // This preserves relative precision MUCH better than a fixed scale.
    let plane_geometry_size = 1000.0 * camera_plane_distance_world;

    // 2D position on the plane.
    let plane_position = (vec2f(f32(v_idx / 2u), f32(v_idx % 2u)) * 2.0 - 1.0) * plane_geometry_size;

    // Make up x and y axis for the plane.
    let plane_y_axis = normalize(cross(config.plane.normal, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), config.plane.normal.x != 0.0)));
    let plane_x_axis = cross(plane_y_axis, config.plane.normal);

    // Move plane geometry with the camera.
    let camera_on_plane = vec2f(dot(plane_x_axis, frame.camera_position), dot(plane_y_axis, frame.camera_position));
    let shifted_plane_position = plane_position + camera_on_plane;

    // Compute world position from shifted plane position.
    let world_position = config.plane.normal * config.plane.distance + plane_x_axis * shifted_plane_position.x + plane_y_axis * shifted_plane_position.y;
    out.position = frame.projection_from_world * vec4f(world_position, 1.0);

    // Determine which "scales" of the grid we want to show. We want to show factor 1, 10, 100, 1000, etc.
    let camera_plane_distance_grid_units = camera_plane_distance_world / config.spacing;
    let line_cardinality = max(log2(camera_plane_distance_grid_units) / log2(10.0) - 0.9, 0.0); // -0.9 instead of 1.0 so we always see a little bit of the next level even if we're very close.
    let line_base_cardinality = floor(line_cardinality);
    let line_spacing_factor = pow(10.0, line_base_cardinality);
    out.next_cardinality_interpolation = line_cardinality - line_base_cardinality;

    return out;
}


// Distance to a grid line in x and y ranging from 0 to 1.
fn calc_distance_to_grid_line(scaled_world_plane_position: vec2f) -> vec2f {
    return 1.0 - abs(fract(scaled_world_plane_position) * 2.0 - 1.0);
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) vec4f {
    // Most basics of determining a basic pixel space grid are very well explained by Ben Golus here: https://bgolus.medium.com/the-best-darn-grid-shader-yet-727f9278b9d8
    // We're not actually implementing the "pristine grid shader" which is a grid with world space thickness,
    // but rather the pixel space grid, which is a lot simpler, but happens to be also described very well in this article.

    // Use a camera ray intersection instead of interpolating the plane vertex positions.
    // Since those can be very far away, this approach ends up being much more precise!
    // (also it has the added benefit that we're independent of the geometry in general!)
    let camera_ray = camera_ray_from_fragcoord(in.position.xy);
    let plane_world_position = intersect_ray_plane(camera_ray, config.plane) * camera_ray.direction + frame.camera_position;
    let plane_y_axis = normalize(cross(config.plane.normal, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), config.plane.normal.x != 0.0)));
    let plane_x_axis = cross(plane_y_axis, config.plane.normal);
    let plane_position = vec2f(dot(plane_x_axis, plane_world_position), dot(plane_y_axis, plane_world_position)) / config.spacing + config.normal_offset;

    // Distance to a grid line in x and y ranging from 0 to 1.
    let distance_to_grid_line_base = calc_distance_to_grid_line(plane_position);

    // Figure out the how wide the lines are in this "draw space".
    let plane_unit_pixel_derivative = fwidthFine(plane_position);
    let line_anti_alias = plane_unit_pixel_derivative;
    let width_in_pixels = config.thickness_ui * frame.pixels_from_point;
    let width_in_grid_units = width_in_pixels * plane_unit_pixel_derivative;
    var intensity_base = linearstep2(width_in_grid_units + line_anti_alias,
                                     width_in_grid_units - line_anti_alias,
                                     distance_to_grid_line_base);

    var fully_invisible_spacing = 2.0; // when lines are this close together, they are invisible
    var fully_visible_spacing = 10.0; // when lines are this far apart, they have full intensity

    if frame.deterministic_rendering == 1 {
        // Fade it out a little bit earlier to reduce numeric noise close to the horizon
        fully_invisible_spacing *= 2.0;
    }

    // Fade lines that get too close to each other.
    // Once the number of pixels per line (== from one line to the next) is below a threshold fade them out.
    //
    // Note that `1/plane_unit_pixel_derivative` would give us more literal pixels per line,
    // but we actually want to know how dense the lines get here so we use `1/width_in_grid_units` instead,
    // such that a value of 1.0 means roughly "100% lines" and 10.0 means "Every 10 pixels there is a lines".
    // Empirically (== making the fade a hard cut and taking screenshot), this works out pretty accurately!
    //
    // Tried smoothstep here, but didn't feel right even with lots of range tweaking.
    let screen_space_line_spacing = 1.0 / max(width_in_grid_units.x, width_in_grid_units.y);
    let grid_closeness_fade = linearstep(fully_invisible_spacing, fully_visible_spacing, screen_space_line_spacing);
    intensity_base *= grid_closeness_fade;

    // Every tenth line is a more intense, we call those "cardinal" lines.
    // Experimented previously with more levels of cardinal lines, but it gets too busy:
    // It seems that if we want to go down this path, we should ensure that there's only two levels of lines on screen at a time.
    let distance_to_grid_line_cardinal = calc_distance_to_grid_line(plane_position * 0.1);
    var intensity_cardinal = linearstep2(width_in_grid_units + line_anti_alias,
                                         width_in_grid_units - line_anti_alias,
                                         distance_to_grid_line_cardinal * 10.0);
    let cardinal_grid_closeness_fade = linearstep(fully_invisible_spacing, fully_visible_spacing, screen_space_line_spacing * 10.0);
    intensity_cardinal *= cardinal_grid_closeness_fade;

    // Combine all lines.
    //
    // Lerp for cardinal & regular.
    // This way we don't break anti-aliasing (as addition would!), mute the regular lines, and make cardinals weaker when there's no regular to support them.
    let cardinal_and_regular = mix(intensity_base, intensity_cardinal, in.next_cardinality_interpolation);

    let intensity_combined = max(cardinal_and_regular.x, cardinal_and_regular.y);

    return config.color * intensity_combined;

    // Useful debugging visualizations:
    // return vec4f(line_cardinality - line_base_cardinality, 0.0, 0.0, 1.0);
    // return vec4f(intensity_combined);
    // return vec4f(grid_closeness_fade, cardinal_grid_closeness_fade, 0.0, 1.0);
}
