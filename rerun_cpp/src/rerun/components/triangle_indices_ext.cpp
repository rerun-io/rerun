#include "triangle_indices.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct TriangleIndicesExt {
            uint32_t indices[3];
#define TriangleIndices TriangleIndicesExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct TriangleIndices from v0/v1/v2 values.
            TriangleIndices(uint32_t v0, uint32_t v1, uint32_t v2) : indices{v0, v1, v2} {}

            /// Construct UVec3D from v0/v1/v2 uint32_t pointer.
            explicit TriangleIndices(const uint32_t* indices)
                : indices{indices[0], indices[1], indices[2]} {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
