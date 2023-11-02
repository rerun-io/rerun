#include "vec2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Vec2DExt {
            float xy[2];
#define Vec2D Vec2DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct Vec2D from x/y values.
            Vec2D(float x, float y) : xy{x, y} {}

            /// Construct Vec2D from x/y float pointer.
            explicit Vec2D(const float* xy_) : xy{xy_[0], xy_[1]} {}

            float x() const {
                return xy[0];
            }

            float y() const {
                return xy[1];
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
