#include "color.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct ColorExt : public Color {
            ColorExt(uint32_t _rgba) : Color(_rgba) {}

#define Color ColorExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Color from unmultiplied RGBA values.
            Color(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255) : rgba(r, g, b, a) {}

            uint8_t r() const {
                return rgba.r();
            }

            uint8_t g() const {
                return rgba.g();
            }

            uint8_t b() const {
                return rgba.b();
            }

            uint8_t a() const {
                return rgba.a();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
