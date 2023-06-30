// The Rerun C++ SDK.

#ifndef RERUN_HPP
#define RERUN_HPP

namespace rerun_c {
#include <rerun.h>
}

namespace rerun {
    inline const char* version_string() {
        return rerun_c::rerun_version_string();
    }
}


#endif // RERUN_HPP
