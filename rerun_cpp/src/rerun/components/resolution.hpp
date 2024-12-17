// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/resolution.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../datatypes/vec2d.hpp"
#include "../rerun_sdk_export.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: Pixel resolution width & height, e.g. of a camera sensor.
    ///
    /// Typically in integer units, but for some use cases floating point may be used.
    struct Resolution {
        rerun::datatypes::Vec2D resolution;

      public: // START of extensions from resolution_ext.cpp:
        /// Construct resolution from width and height floats.
        Resolution(float width, float height) : resolution{width, height} {}

        /// Construct resolution from width and height integers.
        Resolution(int width, int height)
            : resolution{static_cast<float>(width), static_cast<float>(height)} {}

        // END of extensions from resolution_ext.cpp, start of generated code:

      public:
        Resolution() = default;

        Resolution(rerun::datatypes::Vec2D resolution_) : resolution(resolution_) {}

        Resolution& operator=(rerun::datatypes::Vec2D resolution_) {
            resolution = resolution_;
            return *this;
        }

        Resolution(std::array<float, 2> xy_) : resolution(xy_) {}

        Resolution& operator=(std::array<float, 2> xy_) {
            resolution = xy_;
            return *this;
        }

        /// Cast to the underlying Vec2D datatype
        operator rerun::datatypes::Vec2D() const {
            return resolution;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Vec2D) == sizeof(components::Resolution));

    /// \private
    template <>
    struct Loggable<components::Resolution> {
        static constexpr ComponentDescriptor Descriptor = "rerun.components.Resolution";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Vec2D>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::Resolution` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Resolution* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::Vec2D>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::Vec2D>::to_arrow(
                    &instances->resolution,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
