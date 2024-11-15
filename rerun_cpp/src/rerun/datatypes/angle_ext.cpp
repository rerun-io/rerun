#include "angle.hpp"

namespace rerun::datatypes {
#if 0

    // <CODEGEN_COPY_TO_HEADER>

    /// New angle in radians.
    static Angle radians(float radians_) {
        Angle angle;
        angle.angle_radians = radians_;
        return angle;
    }

    /// New angle in degrees.
    ///
    /// Converts to radians to store the angle.
    static Angle degrees(float degrees_) {
        Angle angle;
        // Can't use math constants here: `M_PI` doesn't work on all platforms out of the box and std::numbers::pi is C++20.
        angle.angle_radians = degrees_ * (3.14159265358979323846264338327950288f / 180.f);
        return angle;
    }

    // </CODEGEN_COPY_TO_HEADER>

#endif

} // namespace rerun::datatypes
