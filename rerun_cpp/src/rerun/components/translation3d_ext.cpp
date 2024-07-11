#include "translation3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Translation3DExt {
            float vector[3];
#define Translation3D Translation3DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct Translation3D from x/y/z values.
            Translation3D(float x, float y, float z) : vector{x, y, z} {}

            /// Construct Vec3D from x/y/z float pointer.
            explicit Translation3D(const float* xyz) : vector{xyz[0], xyz[1], xyz[2]} {}

            float x() const {
                return vector.x();
            }

            float y() const {
                return vector.y();
            }

            float z() const {
                return vector.z();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
