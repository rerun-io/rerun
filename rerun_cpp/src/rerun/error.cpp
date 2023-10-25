#include "error.hpp"
#include "c/rerun.h"

#include <arrow/status.h>

#include <algorithm> // For std::transform
#include <cstdlib>   // For getenv
#include <string>

namespace rerun {
    bool is_strict_mode() {
        // MSVC warns if the older `getenv` is used.
        // The new C11 getenv_s on the other hand isn't supported by all C++ compilers.
    #ifdef _MSC_VER
        char env[512] = {};
        size_t env_length = 0;
        if (getenv_s(&env_length, env,  sizeof(env), "RERUN_STRICT") != 0) {
            return false;
        }
    #else
        const char* env = std::getenv("RERUN_STRICT");
        if (env == nullptr) {
            return false;
        }
    #endif

        std::string v = env;
        std::transform(v.begin(), v.end(), v.begin(), [](char c) { return std::tolower(c); });

        if (v == "1" || v == "true" || v == "yes" || v == "on") {
            return true;
        } else if (v == "0" || v == "false" || v == "no" || v == "off") {
            return false;
        } else {
            fprintf(
                stderr,
                "Expected env-var RERUN_STRICT to be 0/1 true/false yes/no on/off, found '%s'",
                env
            );
            return false;
        }
    }

    static StatusLogHandler global_log_handler = nullptr;
    static void* global_log_handler_user_data = nullptr;

    Error::Error(const rr_error& status)
        : code(static_cast<ErrorCode>(status.code)), description(status.description) {}

    Error::Error(const arrow::Status& status) {
        switch (status.code()) {
            case arrow::StatusCode::OK:
                code = ErrorCode::Ok;
                break;
            case arrow::StatusCode::OutOfMemory:
                code = ErrorCode::OutOfMemory;
                break;
            case arrow::StatusCode::KeyError:
                code = ErrorCode::ArrowStatusCode_KeyError;
                break;
            case arrow::StatusCode::TypeError:
                code = ErrorCode::ArrowStatusCode_TypeError;
                break;
            case arrow::StatusCode::Invalid:
                code = ErrorCode::ArrowStatusCode_Invalid;
                break;
            case arrow::StatusCode::IOError:
                code = ErrorCode::ArrowStatusCode_IOError;
                break;
            case arrow::StatusCode::CapacityError:
                code = ErrorCode::ArrowStatusCode_CapacityError;
                break;
            case arrow::StatusCode::IndexError:
                code = ErrorCode::ArrowStatusCode_IndexError;
                break;
            case arrow::StatusCode::Cancelled:
                code = ErrorCode::ArrowStatusCode_Cancelled;
                break;
            case arrow::StatusCode::UnknownError:
                code = ErrorCode::ArrowStatusCode_UnknownError;
                break;
            case arrow::StatusCode::NotImplemented:
                code = ErrorCode::ArrowStatusCode_NotImplemented;
                break;
            case arrow::StatusCode::SerializationError:
                code = ErrorCode::ArrowStatusCode_SerializationError;
                break;
            case arrow::StatusCode::RError:
                code = ErrorCode::ArrowStatusCode_RError;
                break;
            case arrow::StatusCode::CodeGenError:
                code = ErrorCode::ArrowStatusCode_CodeGenError;
                break;
            case arrow::StatusCode::ExpressionValidationError:
                code = ErrorCode::ArrowStatusCode_ExpressionValidationError;
                break;
            case arrow::StatusCode::ExecutionError:
                code = ErrorCode::ArrowStatusCode_ExecutionError;
                break;
            case arrow::StatusCode::AlreadyExists:
                code = ErrorCode::ArrowStatusCode_AlreadyExists;
                break;
            default:
                code = ErrorCode::Unknown;
                break;
        }
        description = status.message();
    }

    void Error::set_log_handler(StatusLogHandler handler, void* userdata) {
        global_log_handler = handler;
        global_log_handler_user_data = userdata;
    }

    void Error::handle() const {
        if (is_ok()) {
            // ok!
        } else if (global_log_handler) {
            global_log_handler(*this, global_log_handler_user_data);
        } else if (is_strict_mode()) {
#ifdef __cpp_exceptions
            throw_on_failure();
#else
            fprintf(stderr, "Rerun ERROR: %s\n", description.c_str());
            abort();
#endif
        } else {
            fprintf(stderr, "Rerun ERROR: %s\n", description.c_str());
        }
    }
} // namespace rerun
