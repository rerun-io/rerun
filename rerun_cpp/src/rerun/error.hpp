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
#define RR_RETURN_NOT_OK(status_expr)              \
    do {                                           \
        const rerun::Error _status_ = status_expr; \
        if (_status_.is_err()) {                   \
            return _status_;                       \
        }                                          \
    } while (false)

namespace rerun {
    /// Status codes returned by the SDK as part of `Status`.
    ///
    /// Category codes are used to group errors together, but are never returned directly.
    enum class ErrorCode : uint32_t {
        Ok = 0x0000'0000,
        OutOfMemory,
        NotImplemented,
        SdkVersionMismatch,

        // Invalid argument errors.
        _CategoryArgument = 0x0000'0010,
        UnexpectedNullArgument,
        InvalidStringArgument,
        InvalidEnumValue,
        InvalidRecordingStreamHandle,
        InvalidSocketAddress,
        InvalidComponentTypeHandle,
        InvalidTensorDimension,
        InvalidArchetypeField,
        FileRead,
        InvalidServerUrl,
        InvalidMemoryLimit,

        // Recording stream errors
        _CategoryRecordingStream = 0x0000'0100,
        RecordingStreamRuntimeFailure,
        RecordingStreamCreationFailure,
        RecordingStreamSaveFailure,
        RecordingStreamStdoutFailure,
        RecordingStreamSpawnFailure,
        RecordingStreamChunkValidationFailure,
        RecordingStreamServeGrpcFailure,

        // Arrow data processing errors.
        _CategoryArrow = 0x0000'1000,
        ArrowFfiSchemaImportError,
        ArrowFfiArrayImportError,

        // Utility errors.
        _CategoryUtilities = 0x0001'0000,
        VideoLoadError,

        // Errors relating to file IO.
        _CategoryFileIO = 0x0010'0000,
        FileOpenFailure,

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

        /// Sets global log handler called for `handle`.
        ///
        /// The default will log to stderr, unless `RERUN_STRICT` is set to something truthy.
        ///
        /// \param handler The handler to call, or `nullptr` to reset to the default.
        /// \param userdata Userdata pointer that will be passed to each invocation of the handler.
        ///
        /// @see log, log_on_failure
        static void set_log_handler(StatusLogHandler handler, void* userdata = nullptr);

        /// Handle this error based on the set log handler.
        ///
        /// If there is no error, nothing happens.
        ///
        /// If you have set a log handler with `set_log_handler`, it will be called.
        /// Else if the `RERUN_STRICT` env-var is set to something truthy,
        /// an exception will be thrown (if `__cpp_exceptions` are enabled),
        /// or the program will abort.
        ///
        /// If no log handler is installed, and we are not in strict mode,
        /// the error will be logged to stderr.
        void handle() const;

        /// Calls the `handle` method and then exits the application with code 1 if the error is not `Ok`.
        /// @see throw_on_failure
        void exit_on_failure() const;

        /// Throws a `std::runtime_error` if the status is not `Ok`.
        ///
        /// If exceptions are disabled, this will forward to `exit_on_failure` instead.
        /// @see exit_on_failure
        void throw_on_failure() const {
#ifdef __cpp_exceptions
            if (is_err()) {
                throw std::runtime_error(description);
            }
#else
            exit_on_failure();
#endif
        }
    };
} // namespace rerun
