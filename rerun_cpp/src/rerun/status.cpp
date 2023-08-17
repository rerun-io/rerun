#include "status.hpp"

#include <rerun.h>

namespace rerun {
    static StatusLogHandler global_log_handler = nullptr;
    static void* global_log_handler_user_data = nullptr;

    Status::Status(const rr_error& status)
        : code(static_cast<StatusCode>(status.code)), description(status.description) {}

    void Status::set_log_handler(StatusLogHandler handler, void* userdata) {
        global_log_handler = handler;
        global_log_handler_user_data = userdata;
    }

    void Status::log_error() const {
        if (global_log_handler) {
            global_log_handler(*this, global_log_handler_user_data);
        } else {
            fprintf(stderr, "%s\n", description.c_str());
        }
    }
} // namespace rerun
