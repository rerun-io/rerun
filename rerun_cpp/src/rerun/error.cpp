#include "error.hpp"

#include <rerun.h>

namespace rerun {
    static StatusLogHandler global_log_handler = nullptr;
    static void* global_log_handler_user_data = nullptr;

    Error::Error(const rr_error& status)
        : code(static_cast<ErrorCode>(status.code)), description(status.description) {}

    void Error::set_log_handler(StatusLogHandler handler, void* userdata) {
        global_log_handler = handler;
        global_log_handler_user_data = userdata;
    }

    void Error::log() const {
        if (global_log_handler) {
            global_log_handler(*this, global_log_handler_user_data);
        } else {
            fprintf(stderr, "%s\n", description.c_str());
        }
    }
} // namespace rerun
