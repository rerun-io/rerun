#include "sdk_info.hpp"
#include <rerun.h>

namespace rerun {
    const char* version_string() {
        return rr_version_string();
    }
} // namespace rerun
