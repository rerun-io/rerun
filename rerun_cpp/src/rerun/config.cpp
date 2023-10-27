#include "config.hpp"

#include <algorithm>
#include <cstdlib>
#include <string>

namespace rerun {

    RerunGlobalConfig& RerunGlobalConfig::instance() {
        static RerunGlobalConfig global;
        return global;
    }

    RerunGlobalConfig::RerunGlobalConfig() : default_enabled(true) {}

} // namespace rerun
