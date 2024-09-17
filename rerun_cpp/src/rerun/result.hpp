#pragma once

#include <type_traits>
#include <utility>

#include "error.hpp"

namespace rerun {
    /// A class for representing either a usable value, or an error.
    ///
    /// In essence a simplified version of rust's Result or arrow's arrow::Result, always using
    /// rerun::Status. For simplicity, the wrapped type must be default constructible.
    template <typename T>
    class [[nodiscard]] Result {
        static_assert(
            std::is_default_constructible<T>::value,
            "Result can only wrap default constructible types."
        );

      public:
        /// Don't allow uninitialized results.
        Result() = delete;

        /// Construct a result from a value, setting error to ok.
        Result(T _value) : value(std::move(_value)), error() {}

        /// Construct a result from an error, default constructing the value.
        Result(rerun::Error _error) : value(), error(std::move(_error)) {}

        /// Construct a result from an arrow status, default constructing the value.
        Result(const arrow::Status& status) : value(), error(status) {}

        /// Construct a result from an arrow status, default constructing the value.
        Result(arrow::Status&& status) : value(), error(std::move(status)) {}

        /// Returns true if error is set to rerun::ErrorCode::Ok, implying that a value is
        /// contained, false otherwise.
        bool is_ok() const {
            return error.is_ok();
        }

        /// Returns true if error is not set to rerun::ErrorCode::Ok, implying that no value is
        /// contained, false otherwise.
        bool is_err() const {
            return error.is_err();
        }

#ifdef __cpp_exceptions
        /// Returns the value if status is ok, throws otherwise.
        const T& value_or_throw() const& {
            error.throw_on_failure();
            return value;
        }

        /// Returns the value if status is ok, throws otherwise.
        T value_or_throw() && {
            error.throw_on_failure();
            return std::move(value);
        }
#endif

      public:
        T value;
        rerun::Error error;
    };
} // namespace rerun
