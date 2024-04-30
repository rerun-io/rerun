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

            /// Construct TriangleIndices from x/y/z values.
            TriangleIndices(uint32_t x, uint32_t y, uint32_t z) : indices{x, y, z} {}

            /// Construct UVec3D from x/y/z uint32_t pointer.
            explicit TriangleIndices(const uint32_t* xyz) : indices{xyz[0], xyz[1], xyz[2]} {}

            uint32_t x() const {
                return indices.x();
            }

            uint32_t y() const {
                return indices.y();
            }

            uint32_t z() const {
                return indices.z();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
