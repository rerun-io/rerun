#include "uvector3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct UVector3DExt {
            uint32_t vector[3];
#define UVector3D UVector3DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct UVector3D from x/y/z values.
            UVector3D(uint32_t x, uint32_t y, uint32_t z) : vector{x, y, z} {}

            /// Construct UVec3D from x/y/z uint32_t pointer.
            explicit UVector3D(const uint32_t* xyz) : vector{xyz[0], xyz[1], xyz[2]} {}

            uint32_t x() const {
                return vector.x();
            }

            uint32_t y() const {
                return vector.y();
            }

            uint32_t z() const {
                return vector.z();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
