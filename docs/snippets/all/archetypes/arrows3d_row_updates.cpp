// Update a set of vectors over time.
//
// See also the `arrows3d_column_updates` example, which achieves the same thing in a single operation.

#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

#include <algorithm>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_arrows3d_row_updates");
    rec.spawn().exit_on_failure();

    // Prepare a fixed sequence of arrows over 5 timesteps.
    // Origins stay constant, vectors change magnitude and direction, and each timestep has a unique color.
    std::vector<std::array<std::array<float, 3>, 5>> origins;
    std::vector<std::array<std::array<float, 3>, 5>> vectors;

    for (size_t i = 0; i < 5; i++) {
        float fi = static_cast<float>(i);

        std::array<std::array<float, 3>, 5> origin;
        std::array<std::array<float, 3>, 5> vector;
        for (size_t j = 0; j < 5; j++) {
            auto fj = static_cast<float>(j);
            auto xs = -1.f + fj * (2.f / 4.f);
            auto zs = fj * (fi / 4.f);

            origin[j] = {xs, xs, 0.0};
            vector[j] = {xs, xs, zs};
        }

        origins.emplace_back(origin);
        vectors.emplace_back(vector);
    }

    // At each timestep, all arrows share the same but changing color.
    std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};

    for (size_t i = 0; i < 5; i++) {
        rec.set_time_duration_secs("time", 10.0 + static_cast<double>(i));
        rec.log(
            "arrows",
            rerun::Arrows3D::from_vectors(vectors[i])
                .with_origins(origins[i])
                .with_colors(colors[i])
        );
    }
}
