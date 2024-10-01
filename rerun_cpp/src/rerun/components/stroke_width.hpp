// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/stroke_width.fbs".

#pragma once

#include "../datatypes/float32.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: The width of a stroke specified in UI points.
    struct StrokeWidth {
        rerun::datatypes::Float32 width;

      public:
        StrokeWidth() = default;

        StrokeWidth(rerun::datatypes::Float32 width_) : width(width_) {}

        StrokeWidth& operator=(rerun::datatypes::Float32 width_) {
            width = width_;
            return *this;
        }

        StrokeWidth(float value_) : width(value_) {}

        StrokeWidth& operator=(float value_) {
            width = value_;
            return *this;
        }

        /// Cast to the underlying Float32 datatype
        operator rerun::datatypes::Float32() const {
            return width;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Float32) == sizeof(components::StrokeWidth));

    /// \private
    template <>
    struct Loggable<components::StrokeWidth> {
        static constexpr const char Name[] = "rerun.components.StrokeWidth";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Float32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::StrokeWidth` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::StrokeWidth* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Float32>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Float32>::to_arrow(
                    &instances->width,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
