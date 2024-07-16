#include "angle.hpp"

namespace rerun::datatypes {
#if 0

    // <CODEGEN_COPY_TO_HEADER>

    /// New angle in radians.
    static Angle radians(float radians_) {
        return Angle { radians_ };
    }

    /// New angle in degrees.
    ///
    /// Converts to radians to store the angle.
    static Angle degrees(float degrees_) {
        // Can't use math constants here: `M_PI` doesn't work on all platforms out of the box and std::numbers::pi is C++20.
        return Angle { degrees_ * (3.14159265358979323846264338327950288f / 180.f) };
    }

    // </CODEGEN_COPY_TO_HEADER>

#endif

} // namespace rerun::datatypes
