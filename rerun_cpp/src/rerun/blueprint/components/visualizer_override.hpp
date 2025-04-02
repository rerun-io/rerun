// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/visualizer_overrides.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/utf8.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: Single visualizer override the visualizers for an entity.
    ///
    /// For details see `archetypes::VisualizerOverrides`.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct VisualizerOverride {
        /// Names of a visualizer that should be active.
        rerun::datatypes::Utf8 visualizer;

      public:
        VisualizerOverride() = default;

        VisualizerOverride(rerun::datatypes::Utf8 visualizer_)
            : visualizer(std::move(visualizer_)) {}

        VisualizerOverride& operator=(rerun::datatypes::Utf8 visualizer_) {
            visualizer = std::move(visualizer_);
            return *this;
        }

        VisualizerOverride(std::string value_) : visualizer(std::move(value_)) {}

        VisualizerOverride& operator=(std::string value_) {
            visualizer = std::move(value_);
            return *this;
        }

        /// Cast to the underlying Utf8 datatype
        operator rerun::datatypes::Utf8() const {
            return visualizer;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::Utf8) == sizeof(blueprint::components::VisualizerOverride)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::VisualizerOverride> {
        static constexpr ComponentDescriptor Descriptor =
            "rerun.blueprint.components.VisualizerOverride";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Utf8>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::VisualizerOverride` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::VisualizerOverride* instances, size_t num_instances
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
                    &instances->visualizer,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
