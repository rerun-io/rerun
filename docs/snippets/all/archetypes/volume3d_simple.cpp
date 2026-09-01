// Log a small volume: a soft sphere sampled on a 32³ grid.

#include <rerun.hpp>

#include <algorithm> // std::max
#include <vector>

constexpr size_t SIZE = 32;
constexpr float RADIUS = 14.0f;

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_volume3d_simple");
    rec.spawn().exit_on_failure();

    // Dimensions are ordered `[z, y, x]`. Only `f16` values are supported for now.
    std::vector<rerun::half> values;
    values.reserve(SIZE * SIZE * SIZE);
    for (size_t z = 0; z < SIZE; ++z) {
        for (size_t y = 0; y < SIZE; ++y) {
            for (size_t x = 0; x < SIZE; ++x) {
                // Squared distance from the center of the grid, in voxel units.
                const float center = 0.5f * static_cast<float>(SIZE - 1);
                const float dx = static_cast<float>(x) - center;
                const float dy = static_cast<float>(y) - center;
                const float dz = static_cast<float>(z) - center;
                const float distance_sq = dx * dx + dy * dy + dz * dz;

                // A soft sphere: 1 at the center, falling off to 0 at `RADIUS`.
                values.push_back(rerun::half::from_float(
                    std::max(0.0f, 1.0f - distance_sq / (RADIUS * RADIUS))
                ));
            }
        }
    }

    rec.log(
        "volume",
        rerun::Volume3D({SIZE, SIZE, SIZE}, values)
            .with_voxel_size(rerun::Vec3D(0.1f, 0.1f, 0.1f))
    );
}
