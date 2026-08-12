#pragma once

#include <cstdint>

// Declared here rather than by including `c/rerun.h`: `half.hpp` is part of the public C++ API,
// and `rerun.hpp` must not leak the C header (see `tests/header_checks.cpp`).
extern "C" uint16_t rr_f16_from_f32(float value);

namespace rerun {
    /// IEEE 754 16-bit half-precision floating point number.
    struct half {
        uint16_t f16;

        /// Converts a 32-bit `float` to a 16-bit `half`, rounding to nearest, ties to even.
        ///
        /// Values too large to represent become infinity, and `NaN` stays `NaN`.
        static half from_float(float value) {
            half result;
            result.f16 = rr_f16_from_f32(value);
            return result;
        }
    };
} // namespace rerun
