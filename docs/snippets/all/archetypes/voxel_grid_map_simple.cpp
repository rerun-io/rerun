// Log a simple sparse voxel grid map.

#include <rerun.hpp>

#include <array>
#include <cstdint>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec =
        rerun::RecordingStream("rerun_example_voxel_grid_map_simple");
    rec.spawn().exit_on_failure();

    const std::vector<rerun::components::VoxelIndex> voxel_indices = {
        rerun::components::VoxelIndex(std::array<int32_t, 3>{-1, 0, 0}),
        rerun::components::VoxelIndex(std::array<int32_t, 3>{1, 0, 0}),
        rerun::components::VoxelIndex(std::array<int32_t, 3>{1, 1, 0}),
        rerun::components::VoxelIndex(std::array<int32_t, 3>{3, 0, 0}),
        rerun::components::VoxelIndex(std::array<int32_t, 3>{3, 0, 1}),
        rerun::components::VoxelIndex(std::array<int32_t, 3>{4, 0, 1}),
    };
    const std::vector<rerun::components::VoxelValue> values = {
        0.0f,
        0.2f,
        0.4f,
        0.6f,
        0.8f,
        1.0f,
    };

    rec.log(
        "world/voxels",
        rerun::archetypes::VoxelGridMap(
            voxel_indices,
            std::array<float, 3>{0.25f, 0.25f, 0.25f}
        )
            .with_values(values)
            .with_value_range(
                rerun::components::ValueRange(std::array<double, 2>{0.0, 1.0})
            )
            .with_colormap(rerun::components::Colormap::Turbo)
            .with_translation({-0.5f, -0.5f, 0.0f})
    );
}
