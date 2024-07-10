// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/visualizer_overrides.fbs".

#pragma once

#include "../../blueprint/datatypes/utf8list.hpp"
#include "../../collection.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: Override the visualizers for an entity.
    ///
    /// This component is a stop-gap mechanism based on the current implementation details
    /// of the visualizer system. It is not intended to be a long-term solution, but provides
    /// enough utility to be useful in the short term.
    ///
    /// The long-term solution is likely to be based off: <https://github.com/rerun-io/rerun/issues/6626>
    ///
    /// This can only be used as part of blueprints. It will have no effect if used
    /// in a regular entity.
    struct VisualizerOverrides {
        /// Names of the visualizers that should be active.
        ///
        /// The built-in visualizers are:
        /// - BarChart
        /// - Arrows2D
        /// - Arrows3D
        /// - Asset3D
        /// - Boxes2D
        /// - Boxes3D
        /// - Cameras
        /// - DepthImage
        /// - Image
        /// - Lines2D
        /// - Lines3D
        /// - Mesh3D
        /// - Points2D
        /// - Points3D
        /// - Transform3DArrows
        /// - Tensor
        /// - TextDocument
        /// - TextLog
        /// - SegmentationImage
        /// - SeriesLine
        /// - SeriesPoint
        rerun::blueprint::datatypes::Utf8List visualizers;

      public:
        VisualizerOverrides() = default;

        VisualizerOverrides(rerun::blueprint::datatypes::Utf8List visualizers_)
            : visualizers(std::move(visualizers_)) {}

        VisualizerOverrides& operator=(rerun::blueprint::datatypes::Utf8List visualizers_) {
            visualizers = std::move(visualizers_);
            return *this;
        }

        VisualizerOverrides(rerun::Collection<std::string> value_)
            : visualizers(std::move(value_)) {}

        VisualizerOverrides& operator=(rerun::Collection<std::string> value_) {
            visualizers = std::move(value_);
            return *this;
        }

        /// Cast to the underlying Utf8List datatype
        operator rerun::blueprint::datatypes::Utf8List() const {
            return visualizers;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::blueprint::datatypes::Utf8List) ==
        sizeof(blueprint::components::VisualizerOverrides)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::VisualizerOverrides> {
        static constexpr const char Name[] = "rerun.blueprint.components.VisualizerOverrides";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::blueprint::datatypes::Utf8List>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::VisualizerOverrides` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::VisualizerOverrides* instances, size_t num_instances
        ) {
            return Loggable<rerun::blueprint::datatypes::Utf8List>::to_arrow(
                &instances->visualizers,
                num_instances
            );
        }
    };
} // namespace rerun
