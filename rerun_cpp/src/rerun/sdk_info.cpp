#include "sdk_info.hpp"
#include <rerun.h>

namespace rr {
    const char* version_string() {
        return rr_version_string();
    }
} // namespace rr
