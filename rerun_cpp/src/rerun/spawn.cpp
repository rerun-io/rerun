#include "spawn.hpp"
#include "c/rerun.h"

namespace rerun {
    Error spawn(
        uint16_t port, const char* memory_limit, const char* executable_name,
        const char* executable_path
    ) {
        rr_spawn_options spawn_opts = {
            port = port,
            memory_limit = memory_limit,
            executable_name = executable_name,
            executable_path = executable_path,
        };
        rr_error error = {};

        rr_spawn(&spawn_opts, &error);

        return Error(error);
    }
} // namespace rerun
