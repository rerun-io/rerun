// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/annotation_context.fbs".

#pragma once

#include "../collection.hpp"
#include "../components/annotation_context.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: The annotation context provides additional information on how to display entities.
    ///
    /// Entities can use `components::ClassId`s and `components::KeypointId`s to provide annotations, and
    /// the labels and colors will be looked up in the appropriate
    /// annotation context. We use the *first* annotation context we find in the
    /// path-hierarchy when searching up through the ancestors of a given entity
    /// path.
    ///
    /// See also `datatypes::ClassDescription`.
    ///
    /// ## Example
    ///
    /// ### Segmentation
    /// ![image](https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm> // fill_n
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_annotation_context_segmentation");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // create an annotation context to describe the classes
    ///     rec.log_static(
    ///         "segmentation",
    ///         rerun::AnnotationContext({
    ///             rerun::AnnotationInfo(1, "red", rerun::Rgba32(255, 0, 0)),
    ///             rerun::AnnotationInfo(2, "green", rerun::Rgba32(0, 255, 0)),
    ///         })
    ///     );
    ///
    ///     // create a segmentation image
    ///     const int HEIGHT = 200;
    ///     const int WIDTH = 300;
    ///     std::vector<uint8_t> data(WIDTH * HEIGHT, 0);
    ///     for (auto y = 50; y <100; ++y) {
    ///         std::fill_n(data.begin() + y * WIDTH + 50, 70, static_cast<uint8_t>(1));
    ///     }
    ///     for (auto y = 100; y <180; ++y) {
    ///         std::fill_n(data.begin() + y * WIDTH + 130, 150, static_cast<uint8_t>(2));
    ///     }
    ///
    ///     rec.log("segmentation/image", rerun::SegmentationImage(data.data(), {WIDTH, HEIGHT}));
    /// }
    /// ```
    struct AnnotationContext {
        /// List of class descriptions, mapping class indices to class names, colors etc.
        rerun::components::AnnotationContext context;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.AnnotationContextIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        AnnotationContext() = default;
        AnnotationContext(AnnotationContext&& other) = default;

        explicit AnnotationContext(rerun::components::AnnotationContext _context)
            : context(std::move(_context)) {}
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::AnnotationContext> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const archetypes::AnnotationContext& archetype
        );
    };
} // namespace rerun
