// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/depth_meter.fbs".

#pragma once

#include "../component_descriptor.hpp"
#include "../datatypes/float32.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: The world->depth map scaling factor.
    ///
    /// This measures how many depth map units are in a world unit.
    /// For instance, if a depth map uses millimeters and the world uses meters,
    /// this value would be `1000`.
    ///
    /// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
    /// In 3D views on the other hand, this affects where the points of the point cloud are placed.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct DepthMeter {
        rerun::datatypes::Float32 value;

      public:
        DepthMeter() = default;

        DepthMeter(rerun::datatypes::Float32 value_) : value(value_) {}

        DepthMeter& operator=(rerun::datatypes::Float32 value_) {
            value = value_;
            return *this;
        }

        DepthMeter(float value_) : value(value_) {}

        DepthMeter& operator=(float value_) {
            value = value_;
            return *this;
        }

        /// Cast to the underlying Float32 datatype
        operator rerun::datatypes::Float32() const {
            return value;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Float32) == sizeof(components::DepthMeter));

    /// \private
    template <>
    struct Loggable<components::DepthMeter> {
        static constexpr ComponentDescriptor Descriptor = "rerun.components.DepthMeter";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Float32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::DepthMeter` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::DepthMeter* instances, size_t num_instances
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
                    &instances->value,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
