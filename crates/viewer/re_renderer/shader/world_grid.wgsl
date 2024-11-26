#import <./global_bindings.wgsl>

struct WorldGridUniformBuffer {
    color: vec4f,

    /// A value of [`super::GridPlane`]
    orientation: u32,

    /// How far apart the closest sets of lines are.
    spacing: f32,

    /// How thick the lines are in UI units.
    thickness_ui: f32,
}

@group(1) @binding(0)
var<uniform> config: WorldGridUniformBuffer;

// See world_grid::GridPlane
const ORIENTATION_XZ: u32 = 0;
const ORIENTATION_YZ: u32 = 1;
const ORIENTATION_XY: u32 = 2;

struct VertexOutput {
    @builtin(position)
    position: vec4f,

    @location(0)
    scaled_world_plane_position: vec2f,
};

const PLANE_SCALE: f32 = 10000.0;

// Spans a large quad where centered around the camera.
//
// This gives us the "canvas" to drawn the grid on.
// Compared to a fullscreen pass, we get the z value (and thus early z testing) for free,
// as well as never covering the screen above the horizon.
@vertex
fn main_vs(@builtin(vertex_index) v_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    var plane_position = (vec2f(f32(v_idx / 2u), f32(v_idx % 2u)) * 2.0 - 1.0) * PLANE_SCALE;
    var world_position: vec3f;
    switch (config.orientation) {
        case ORIENTATION_XZ: {
            plane_position += frame.camera_position.xz;
            world_position = vec3f(plane_position.x, 0.0, plane_position.y);
        }
        case ORIENTATION_YZ: {
            plane_position += frame.camera_position.yz;
            world_position = vec3f(0.0, plane_position.x, plane_position.y);
        }
        case ORIENTATION_XY: {
            plane_position += frame.camera_position.xy;
            world_position = vec3f(plane_position.x, plane_position.y, 0.0);
        }
        default: {
            world_position = vec3f(0.0);
        }
    }

    out.position = frame.projection_from_world * vec4f(world_position, 1.0);
    out.scaled_world_plane_position = plane_position / config.spacing;

    return out;
}

// Like smoothstep, but linear.
// Used for antialiasing: smoothstep works as well but is subtly worse.
fn linearstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    return saturate((x - edge0) / (edge1 - edge0));
}
fn linearstep2(edge0: vec2f, edge1: vec2f, x: vec2f) -> vec2f {
    return saturate((x - edge0) / (edge1 - edge0));
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) vec4f {
    // The basics are very well explained by Ben Golus here: https://bgolus.medium.com/the-best-darn-grid-shader-yet-727f9278b9d8
    // We're not actually implementing the "pristine grid shader" which is a world space grid,
    // but rather the pixel space grid, which is a lot simpler, but happens to be also described very well in this article.

    // Distance to a grid line in x and y ranging from 0 to 1.
    let distance_to_grid_line = 1.0 - abs(fract(in.scaled_world_plane_position) * 2.0 - 1.0);

    // Figure out the how wide the lines are in this "draw space".
    let plane_unit_pixel_derivative = fwidthFine(in.scaled_world_plane_position);
    let line_anti_alias = plane_unit_pixel_derivative;
    let width_in_pixels = config.thickness_ui * frame.pixels_from_point;
    let width_in_grid_units = width_in_pixels * plane_unit_pixel_derivative;
    var intensity_regular = linearstep2(width_in_grid_units + line_anti_alias, width_in_grid_units - line_anti_alias, distance_to_grid_line);

    // Fade lines that get too close to each other.
    // Once the number of pixels per unit (== from one line to the next) is below a threshold fade them out.
    // Tried smoothstep here, but didn't feel right even with lots of range tweaking.
    let pixel_per_plane_unit = 1.0 / max(width_in_grid_units.x, width_in_grid_units.y);
    let grid_closeness_fade = linearstep(1.0, 10.0, pixel_per_plane_unit);
    intensity_regular *= grid_closeness_fade;

    // Every tenth line is a more intense.
    let distance_to_grid_line_cardinal = 1.0 - abs(fract(in.scaled_world_plane_position * 0.1) * 2.0 - 1.0);
    var cardinal_line_intensity = linearstep2(width_in_grid_units + line_anti_alias, width_in_grid_units - line_anti_alias, distance_to_grid_line_cardinal * 10.0);
    let cardinal_grid_closeness_fade = linearstep(2.0, 10.0, pixel_per_plane_unit * 10.0);
    cardinal_line_intensity *= cardinal_grid_closeness_fade;

    // Fade on accute viewing angles.
    // TODO:

    // Combine all lines.
    //
    // Lerp for cardinal & regular.
    // This way we don't break anti-aliasing (as addition would!), mute the regular lines, and make cardinals weaker when there's no regular to support them.
    let cardinal_and_regular = mix(intensity_regular, cardinal_line_intensity, 0.6);
    // X and Y are combined like akin to premultiplied alpha operations.
    let intensity_combined = saturate(cardinal_and_regular.x * (1.0 - cardinal_and_regular.y) + cardinal_and_regular.y);


    return config.color * intensity_combined;

    // Debugging visualizations:
    //return vec4f(intensity_combined);
    //return vec4f(grid_closeness_fade, cardinal_grid_closeness_fade, 0.0, 1.0);
}
