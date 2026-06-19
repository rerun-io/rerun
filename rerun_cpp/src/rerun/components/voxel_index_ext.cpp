#include "voxel_index.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct VoxelIndexExt {
            int32_t index[3];
#define VoxelIndex VoxelIndexExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct VoxelIndex from x/y/z values.
            VoxelIndex(int32_t x, int32_t y, int32_t z) : index{x, y, z} {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
