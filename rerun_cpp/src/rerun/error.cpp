#include "error.hpp"

#include <arrow/status.h>
#include <rerun.h>

namespace rerun {
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

    void Error::log() const {
        if (global_log_handler) {
            global_log_handler(*this, global_log_handler_user_data);
        } else {
            fprintf(stderr, "ERROR: %s\n", description.c_str());
        }
    }
} // namespace rerun
