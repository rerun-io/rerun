// General information about the SDK.
#pragma once

#include "error.hpp"

namespace rerun {
    /// The Rerun C++ SDK version as a human-readable string.
    const char* version_string();

    /// Checks whether the version reported by the rerun_c binary matches `sdk_version_string`.
    ///
    /// This method is called on various C++ API entry points, calling `Error::handle` on the return value.
    Error check_binary_and_header_version_match();
} // namespace rerun
