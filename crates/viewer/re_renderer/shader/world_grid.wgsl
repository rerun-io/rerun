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
}

@group(1) @binding(0)
var<uniform> config: WorldGridUniformBuffer;

struct VertexOutput {
    @builtin(position)
    position: vec4f,

    @location(0)
    scaled_world_plane_position: vec2f,
};

// We have to make up some world space geometry which then necessarily gets a limited size.
// Putting a too high number here makes things break down because of floating point inaccuracies.
// But arguably at that point we're potentially doomed either way since precision will break down in other parts of the rendering as well.
//
// This is the main drawback of the plane approach over the screen space filling one.
const PLANE_GEOMETRY_SIZE: f32 = 10000.0;

// Spans a large quad where centered around the camera.
//
// This gives us the "canvas" to drawn the grid on.
// Compared to a fullscreen pass, we get the z value (and thus early z testing) for free,
// as well as never covering the screen above the horizon.
@vertex
fn main_vs(@builtin(vertex_index) v_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    var plane_position = (vec2f(f32(v_idx / 2u), f32(v_idx % 2u)) * 2.0 - 1.0) * PLANE_GEOMETRY_SIZE;

    // Make up x and y axis for the plane.
    let plane_y_axis = normalize(cross(config.plane.normal, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), config.plane.normal.x != 0.0)));
    let plane_x_axis = cross(plane_y_axis, config.plane.normal);

    // Move plane geometry with the camera.
    let camera_on_plane = vec2f(dot(plane_x_axis, frame.camera_position), dot(plane_y_axis, frame.camera_position));
    let shifted_plane_position = plane_position + camera_on_plane;

    // Compute world position from shifted plane position.
    let world_position = config.plane.normal * -config.plane.distance + plane_x_axis * shifted_plane_position.x + plane_y_axis * shifted_plane_position.y;

    out.position = frame.projection_from_world * vec4f(world_position, 1.0);
    out.scaled_world_plane_position = shifted_plane_position / config.spacing;

    return out;
}


// Distance to a grid line in x and y ranging from 0 to 1.
fn calc_distance_to_grid_line(scaled_world_plane_position: vec2f) -> vec2f {
    return 1.0 - abs(fract(scaled_world_plane_position) * 2.0 - 1.0);
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) vec4f {
    // Most basics are very well explained by Ben Golus here: https://bgolus.medium.com/the-best-darn-grid-shader-yet-727f9278b9d8
    // We're not actually implementing the "pristine grid shader" which is a grid with world space thickness,
    // but rather the pixel space grid, which is a lot simpler, but happens to be also described very well in this article.

    // Distance to a grid line in x and y ranging from 0 to 1.
    let distance_to_grid_line = calc_distance_to_grid_line(in.scaled_world_plane_position);

    // Figure out the how wide the lines are in this "draw space".
    let plane_unit_pixel_derivative = fwidthFine(in.scaled_world_plane_position);
    let line_anti_alias = plane_unit_pixel_derivative;
    let width_in_pixels = config.thickness_ui * frame.pixels_from_point;
    let width_in_grid_units = width_in_pixels * plane_unit_pixel_derivative;
    var intensity_regular = linearstep2(width_in_grid_units + line_anti_alias, width_in_grid_units - line_anti_alias, distance_to_grid_line);

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
    let grid_closeness_fade = linearstep(1.0, 10.0, screen_space_line_spacing);
    intensity_regular *= grid_closeness_fade;

    // Every tenth line is a more intense, we call those "cardinal" lines.
    // Experimented previously with more levels of cardinal lines, but it gets too busy:
    // It seems that if we want to go down this path, we should ensure that there's only two levels of lines on screen at a time.
    const CARDINAL_LINE_FACTOR: f32 = 10.0;
    let distance_to_grid_line_cardinal = calc_distance_to_grid_line(in.scaled_world_plane_position * (1.0 / CARDINAL_LINE_FACTOR));
    var cardinal_line_intensity = linearstep2(width_in_grid_units + line_anti_alias, width_in_grid_units - line_anti_alias,
                                              distance_to_grid_line_cardinal * CARDINAL_LINE_FACTOR);
    let cardinal_grid_closeness_fade = linearstep(2.0, 10.0, screen_space_line_spacing * CARDINAL_LINE_FACTOR); // Fade cardinal lines a little bit earlier (because it looks nicer)
    cardinal_line_intensity *= cardinal_grid_closeness_fade;

    // Combine all lines.
    //
    // Lerp for cardinal & regular.
    // This way we don't break anti-aliasing (as addition would!), mute the regular lines, and make cardinals weaker when there's no regular to support them.
    let cardinal_and_regular = mix(intensity_regular, cardinal_line_intensity, 0.4);
    // X and Y are combined like akin to premultiplied alpha operations.
    let intensity_combined = saturate(cardinal_and_regular.x * (1.0 - cardinal_and_regular.y) + cardinal_and_regular.y);


    return config.color * intensity_combined;

    // Useful debugging visualizations:
    //return vec4f(intensity_combined);
    //return vec4f(grid_closeness_fade, cardinal_grid_closeness_fade, 0.0, 1.0);
}
