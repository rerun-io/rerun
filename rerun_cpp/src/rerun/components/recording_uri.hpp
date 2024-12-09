// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/recording_uri.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../datatypes/utf8.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::components {
    /// **Component**: A recording URI (Uniform Resource Identifier).
    struct RecordingUri {
        rerun::datatypes::Utf8 recording_uri;

      public:
        RecordingUri() = default;

        RecordingUri(rerun::datatypes::Utf8 recording_uri_)
            : recording_uri(std::move(recording_uri_)) {}

        RecordingUri& operator=(rerun::datatypes::Utf8 recording_uri_) {
            recording_uri = std::move(recording_uri_);
            return *this;
        }

        RecordingUri(std::string value_) : recording_uri(std::move(value_)) {}

        RecordingUri& operator=(std::string value_) {
            recording_uri = std::move(value_);
            return *this;
        }

        /// Cast to the underlying Utf8 datatype
        operator rerun::datatypes::Utf8() const {
            return recording_uri;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Utf8) == sizeof(components::RecordingUri));

    /// \private
    template <>
    struct Loggable<components::RecordingUri> {
        static constexpr ComponentDescriptor Descriptor = "rerun.components.RecordingUri";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Utf8>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::RecordingUri` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::RecordingUri* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Utf8>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Utf8>::to_arrow(
                    &instances->recording_uri,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
