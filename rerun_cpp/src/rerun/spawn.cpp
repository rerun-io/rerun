#include "spawn.hpp"
#include "c/rerun.h"
#include "string_utils.hpp"

namespace rerun {
    Error spawn(
        uint16_t port, std::string_view memory_limit, std::string_view executable_name,
        std::optional<std::string_view> executable_path
    ) {
        rr_spawn_options spawn_opts;
        spawn_opts.port = port;
        spawn_opts.memory_limit = detail::to_rr_string(memory_limit);
        spawn_opts.executable_name = detail::to_rr_string(executable_name);
        spawn_opts.executable_path = detail::to_rr_string(executable_path);
        rr_error error = {};

        rr_spawn(&spawn_opts, &error);

        return Error(error);
    }
} // namespace rerun
