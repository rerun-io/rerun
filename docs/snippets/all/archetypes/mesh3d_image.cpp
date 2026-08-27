// Log an image as a 3D quad.

#include <rerun.hpp>

#include <algorithm>
#include <random>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_mesh3d_image");
    rec.spawn().exit_on_failure();

    const uint32_t width = 256;
    const uint32_t height = 256;

    // Simple gradient image
    std::vector<uint8_t> image(width * height * 3);
    for (uint32_t y = 0; y < height; ++y) {
        for (uint32_t x = 0; x < width; ++x) {
            image[(y * width + x) * 3 + 0] = static_cast<uint8_t>(x);
            image[(y * width + x) * 3 + 1] = static_cast<uint8_t>(y);
            image[(y * width + x) * 3 + 2] = 0;
        }
    }

    const rerun::Position3D top_left = {1.0f, 1.0f, 1.0f};
    const rerun::Position3D top_right = {1.0f, 0.0f, 1.0f};
    const rerun::Position3D bottom_right = {1.0f, 0.0f, 0.0f};
    const rerun::Position3D bottom_left = {1.0f, 1.0f, 0.0f};
    const uint8_t alpha = 255;

    // Inset by half a pixel so the opposite edges of the image don't leak onto the border.
    const float u0 = 0.5f / static_cast<float>(width);
    const float v0 = 0.5f / static_cast<float>(height);
    const float u1 = 1.0f - u0;
    const float v1 = 1.0f - v0;

    rec.log(
        "image",
        rerun::Mesh3D({top_left, top_right, bottom_right, bottom_left})
            .with_vertex_texcoords({{u0, v0}, {u1, v0}, {u1, v1}, {u0, v1}})
            .with_triangle_indices({{0, 2, 1}, {0, 3, 2}})
            .with_albedo_texture_buffer(rerun::ImageBuffer(image))
            .with_albedo_texture_format(rerun::components::ImageFormat(
                {width, height},
                rerun::ColorModel::RGB,
                rerun::ChannelDatatype::U8
            ))
            .with_albedo_factor(rerun::Rgba32(255, 255, 255, alpha))
    );

    // Example 3D data
    std::default_random_engine gen(1);
    std::uniform_real_distribution<float> uniform(0.1f, 0.9f);
    std::normal_distribution<float> normal_dist(0.0f, 0.01f);
    // On MSVC uint8_t distributions are not supported.
    std::uniform_int_distribution<int> color_dist(40, 254);

    std::vector<rerun::Collection<rerun::Vec3D>> strips;
    std::vector<rerun::Color> colors;
    strips.reserve(20);
    colors.reserve(20);

    for (int i = 0; i < 20; ++i) {
        float u = uniform(gen);
        float v = uniform(gen);
        colors.emplace_back(
            static_cast<uint8_t>(color_dist(gen)),
            static_cast<uint8_t>(color_dist(gen)),
            static_cast<uint8_t>(color_dist(gen))
        );

        std::vector<rerun::Vec3D> strip;
        strip.reserve(40);
        for (int step = 0; step < 40; ++step) {
            float t = static_cast<float>(step) / static_cast<float>(40 - 1);
            u = std::clamp(u + normal_dist(gen), 0.0f, 1.0f);
            v = std::clamp(v + normal_dist(gen), 0.0f, 1.0f);
            strip.push_back({1.0f - t, 1.0f - u, 1.0f - v});
        }
        strips.emplace_back(std::move(strip));
    }

    rec.log(
        "tracks",
        rerun::LineStrips3D(strips).with_colors(colors).with_radii(0.002f)
    );

    rec.log(
        "time-axis",
        rerun::Arrows3D::from_vectors({{1.0f, 0.0f, 0.0f}})
            .with_origins({{0.0f, 1.0f, 0.0f}})
            .with_labels({"Time"})
            .with_colors({{255, 255, 255}})
            .with_radii({0.005f})
    );
}
