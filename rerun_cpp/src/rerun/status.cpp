#include "status.hpp"

#include <rerun.h>

namespace rerun {
    Status::Status(const rr_status& status)
        : code(static_cast<StatusCode>(status.code)), description(status.description) {}
} // namespace rerun
