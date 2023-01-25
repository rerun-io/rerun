// Converts float from 0.0..=1.0 into a color using Turbo.
//
// The Turbo color map described here:
// https://ai.googleblog.com/2019/08/turbo-improved-rainbow-colormap-for.html
//
// Turbo color map is originally a lookup table!
// I.e. for any value not captured we'd need to interpolate.
//
// Instead, we use this polynomial approximation.
// https://gist.github.com/mikhailov-work/0d177465a8151eb6ede1768d51d476c7
#[allow(clippy::excessive_precision)]
pub fn turbo_color_map(x: f32) -> egui::Color32 {
    let red_vec4 = glam::vec4(0.13572138, 4.61539260, -42.66032258, 132.13108234);
    let green_vec4 = glam::vec4(0.09140261, 2.19418839, 4.84296658, -14.18503333);
    let blue_vec4 = glam::vec4(0.10667330, 12.64194608, -60.58204836, 110.36276771);
    let red_vec2 = glam::vec2(-152.94239396, 59.28637943);
    let green_vec2 = glam::vec2(4.27729857, 2.82956604);
    let blue_vec2 = glam::vec2(-89.90310912, 27.34824973);

    let v4 = glam::vec4(1.0, x, x * x, x * x * x);
    let v2 = glam::vec2(v4.z, v4.w) * v4.z;

    // Above sources are not explicit about it but this color is seemingly already in srgb gamma space.
    egui::Color32::from_rgb(
        ((v4.dot(red_vec4) + v2.dot(red_vec2)) * 255.0) as u8,
        ((v4.dot(green_vec4) + v2.dot(green_vec2)) * 255.0) as u8,
        ((v4.dot(blue_vec4) + v2.dot(blue_vec2)) * 255.0) as u8,
    )
}
