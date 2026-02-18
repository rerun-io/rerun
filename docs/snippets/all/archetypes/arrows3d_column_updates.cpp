// Update a set of vectors over time, in a single operation.
//
// This is semantically equivalent to the `arrows3d_row_updates` example, albeit much faster.

#include <rerun.hpp>

#include <algorithm>
#include <vector>

using namespace std::chrono_literals;

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_arrows3d_column_updates");
    rec.spawn().exit_on_failure();

    // Prepare a fixed sequence of arrows over 5 timesteps.
    // Origins stay constant, vectors change magnitude and direction, and each timestep has a unique color.
    std::vector<std::array<float, 3>> origins;
    std::vector<std::array<float, 3>> vectors;

    for (size_t i = 0; i < 5; i++) {
        float fi = static_cast<float>(i);

        for (size_t j = 0; j < 5; j++) {
            auto fj = static_cast<float>(j);
            auto xs = -1.f + fj * (2.f / 4.f);
            auto zs = fj * (fi / 4.f);

            std::array<float, 3> origin = {xs, xs, 0.f};
            std::array<float, 3> vector = {xs, xs, zs};

            origins.emplace_back(origin);
            vectors.emplace_back(vector);
        }
    }

    // At each timestep, all arrows share the same but changing color.
    std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};

    // Log at seconds 10-14
    auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
    auto time_column = rerun::TimeColumn::from_durations("time", std::move(times));

    auto arrows =
        rerun::Arrows3D().with_origins(origins).with_vectors(vectors).columns({5, 5, 5, 5, 5});

    rec.send_columns(
        "arrows",
        time_column,
        arrows,
        rerun::Arrows3D::update_fields().with_colors(colors).columns()
    );
}
