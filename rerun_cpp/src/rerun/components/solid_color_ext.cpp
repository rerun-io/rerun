#include "solid_color.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct SolidColorExt : public SolidColor {
            SolidColorExt(uint32_t _rgba) : SolidColor(_rgba) {}

#define SolidColor SolidColorExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct SolidColor from unmultiplied RGBA values.
            SolidColor(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255) : rgba(r, g, b, a) {}

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

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
