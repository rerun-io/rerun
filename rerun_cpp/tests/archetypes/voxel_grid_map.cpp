#include "archetype_test.hpp"

#include <rerun/archetypes/voxel_grid_map.hpp>

#include <array>
#include <cstdint>
#include <vector>

using namespace rerun;
using namespace rerun::archetypes;

#define TEST_TAG "[voxel_grid_map][archetypes]"

SCENARIO(
    "VoxelGridMap archetype can be serialized with the same result for manually built instances "
    "and the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        const std::vector<components::VoxelIndex> voxel_indices = {
            components::VoxelIndex(std::array<int32_t, 3>{-1, 0, 2}),
            components::VoxelIndex(std::array<int32_t, 3>{2, 1, 3}),
        };

        auto from_builder =
            VoxelGridMap(voxel_indices, 0.25f)
                .with_values({0.1f, 0.9f})
                .with_colors({0xAA0000CC, 0x00BB00DD})
                .with_translation({1.0f, 2.0f, 3.0f})
                .with_opacity(0.5f)
                .with_value_range(components::ValueRange(std::array<double, 2>{0.0, 1.0}))
                .with_colormap(components::Colormap::Turbo);

        VoxelGridMap from_manual;
        from_manual.voxel_indices = ComponentBatch::from_loggable<components::VoxelIndex>(
                                        Collection<components::VoxelIndex>(voxel_indices),
                                        VoxelGridMap::Descriptor_voxel_indices
        )
                                        .value_or_throw();
        from_manual.cell_size = ComponentBatch::from_loggable(
                                    components::CellSize(0.25f),
                                    VoxelGridMap::Descriptor_cell_size
        )
                                    .value_or_throw();
        from_manual.values = ComponentBatch::from_loggable<components::VoxelValue>(
                                 {0.1f, 0.9f},
                                 VoxelGridMap::Descriptor_values
        )
                                 .value_or_throw();
        from_manual.colors = ComponentBatch::from_loggable<components::Color>(
                                 {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                 VoxelGridMap::Descriptor_colors
        )
                                 .value_or_throw();
        from_manual.translation = ComponentBatch::from_loggable(
                                      components::Translation3D(1.0f, 2.0f, 3.0f),
                                      VoxelGridMap::Descriptor_translation
        )
                                      .value_or_throw();
        from_manual.opacity = ComponentBatch::from_loggable(
                                  components::Opacity(0.5f),
                                  VoxelGridMap::Descriptor_opacity
        )
                                  .value_or_throw();
        from_manual.value_range = ComponentBatch::from_loggable(
                                      components::ValueRange(std::array<double, 2>{0.0, 1.0}),
                                      VoxelGridMap::Descriptor_value_range
        )
                                      .value_or_throw();
        from_manual.colormap = ComponentBatch::from_loggable(
                                   components::Colormap::Turbo,
                                   VoxelGridMap::Descriptor_colormap
        )
                                   .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
