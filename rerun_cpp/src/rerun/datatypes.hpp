// Deprecated alias for `rerun/encodings.hpp`.
//
// `datatypes` was renamed to `encodings` in 0.37, because it clashed with the Arrow `DataType`.

#pragma once

#include "encodings.hpp"

namespace rerun {
    /// \deprecated Renamed to `rerun::encodings` in 0.37.
    namespace datatypes = encodings;
} // namespace rerun
