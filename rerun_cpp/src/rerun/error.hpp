#pragma once

#include <cstdint>
#include <string>

#ifdef __cpp_exceptions
#include <stdexcept>
#endif

namespace arrow {
    class Status;
}

struct rr_error;

/// Return error if a given rerun::Error producing expression is not rerun::ErrorCode::Ok.
#define RR_RETURN_NOT_OK(status_expr)      \
    do {                                   \
        const auto _status_ = status_expr; \
        if (_status_.is_err()) {           \
            return _status_;               \
        }                                  \
    } while (false)

namespace rerun {
    /// Status codes returned by the SDK as part of `Status`.
    ///
    /// Category codes are used to group errors together, but are never returned directly.
    enum class ErrorCode : uint32_t {
        Ok = 0x0000'0000,
        OutOfMemory = 0x0000'0001,
        NotImplemented = 0x0000'0002,

        // Invalid argument errors.
        _CategoryArgument = 0x0000'0010,
        UnexpectedNullArgument,
        InvalidStringArgument,
        InvalidRecordingStreamHandle,
        InvalidSocketAddress,

        // Recording stream errors
        _CategoryRecordingStream = 0x0000'0100,
        RecordingStreamCreationFailure,
        RecordingStreamSaveFailure,

        // Arrow data processing errors.
        _CategoryArrow = 0x0000'1000,
        ArrowIpcMessageParsingFailure,
        ArrowDataCellError,

        // Errors directly translated from arrow::StatusCode.
        _CategoryArrowCppStatus = 0x1000'0000,
        ArrowStatusCode_KeyError,
        ArrowStatusCode_TypeError,
        ArrowStatusCode_Invalid,
        ArrowStatusCode_IOError,
        ArrowStatusCode_CapacityError,
        ArrowStatusCode_IndexError,
        ArrowStatusCode_Cancelled,
        ArrowStatusCode_UnknownError,
        ArrowStatusCode_NotImplemented,
        ArrowStatusCode_SerializationError,
        ArrowStatusCode_RError,
        ArrowStatusCode_CodeGenError,
        ArrowStatusCode_ExpressionValidationError,
        ArrowStatusCode_ExecutionError,
        ArrowStatusCode_AlreadyExists,

        Unknown = 0xFFFF'FFFF,
    };

    /// Callback function type for log handlers.
    using StatusLogHandler = void (*)(const class Error& status, void* userdata);

    /// Status outcome object (success or error) returned for fallible operations.
    ///
    /// Converts to `true` for success, `false` for failure.
    class [[nodiscard]] Error {
      public:
        /// Result code for the given operation.
        ErrorCode code = ErrorCode::Ok;

        /// Human readable description of the error.
        std::string description;

      public:
        Error() = default;

        Error(ErrorCode _code, std::string _description)
            : code(_code), description(std::move(_description)) {}

        /// Construct from a C status object.
        Error(const rr_error& status);

        /// Construct from an arrow status.
        Error(const arrow::Status& status);

        /// Creates a new error set to ok.
        static Error ok() {
            return Error();
        }

        /// Compare two errors for equality. Requires the description to match.
        bool operator==(const Error& other) const {
            return code == other.code && description == other.description;
        }

        /// Returns true if the code is `Ok`.
        bool is_ok() const {
            return code == ErrorCode::Ok;
        }

        /// Returns true if the code is not `Ok`.
        bool is_err() const {
            return code != ErrorCode::Ok;
        }

        /// Sets global log handler called for `log` and `log_on_failure`.
        ///
        /// The default will log to stderr.
        ///
        /// @param handler The handler to call, or `nullptr` to reset to the default.
        /// @param userdata Userdata pointer that will be passed to each invocation of the handler.
        ///
        /// @see log, log_on_failure
        static void set_log_handler(StatusLogHandler handler, void* userdata = nullptr);

        /// Logs this status via the global log handler.
        ///
        /// @see set_log_handler
        void log() const;

        /// Logs this status if failed via the global log handler.
        ///
        /// @see set_log_handler
        void log_on_failure() const {
            if (is_err()) {
                log();
            }
        }

#ifdef __cpp_exceptions
        /// Throws a `std::runtime_error` if the status is not `Ok`.
        void throw_on_failure() const {
            if (is_err()) {
                throw std::runtime_error(description);
            }
        }
#endif
    };
} // namespace rerun
