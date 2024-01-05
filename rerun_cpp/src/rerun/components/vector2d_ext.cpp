#include "vector2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Vector2DExt {
            float vector[2];
#define Vector2D Vector2DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct Vector2D from x/y values.
            Vector2D(float x, float y) : vector{x, y} {}

            /// Construct Vec2D from x/y/z float pointer.
            explicit Vector2D(const float* xyz) : vector{xyz[0], xyz[1]} {}

            float x() const {
                return vector.x();
            }

            float y() const {
                return vector.y();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
