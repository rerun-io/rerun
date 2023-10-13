#pragma once

#include <cstdint>

namespace rerun {
    /// IEEE 754 16-bit half-precision floating point number.
    struct half {
        uint16_t f16;
    };
} // namespace rerun
