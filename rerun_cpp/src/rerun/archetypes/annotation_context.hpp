// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/annotation_context.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/annotation_context.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
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
    /// ![image](https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/full.png)
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
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct AnnotationContext {
        /// List of class descriptions, mapping class indices to class names, colors etc.
        std::optional<ComponentBatch> context;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.AnnotationContextIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.AnnotationContext";

        /// `ComponentDescriptor` for the `context` field.
        static constexpr auto Descriptor_context = ComponentDescriptor(
            ArchetypeName, "context",
            Loggable<rerun::components::AnnotationContext>::Descriptor.component_name
        );

      public:
        AnnotationContext() = default;
        AnnotationContext(AnnotationContext&& other) = default;
        AnnotationContext(const AnnotationContext& other) = default;
        AnnotationContext& operator=(const AnnotationContext& other) = default;
        AnnotationContext& operator=(AnnotationContext&& other) = default;

        explicit AnnotationContext(rerun::components::AnnotationContext _context)
            : context(ComponentBatch::from_loggable(std::move(_context), Descriptor_context)
                          .value_or_throw()) {}

        /// Update only some specific fields of a `AnnotationContext`.
        static AnnotationContext update_fields() {
            return AnnotationContext();
        }

        /// Clear all the fields of a `AnnotationContext`.
        static AnnotationContext clear_fields();

        /// List of class descriptions, mapping class indices to class names, colors etc.
        AnnotationContext with_context(const rerun::components::AnnotationContext& _context) && {
            context = ComponentBatch::from_loggable(_context, Descriptor_context).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `context` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_context` should
        /// be used when logging a single row's worth of data.
        AnnotationContext with_many_context(
            const Collection<rerun::components::AnnotationContext>& _context
        ) && {
            context = ComponentBatch::from_loggable(_context, Descriptor_context).value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentBatch::partitioned`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
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
        static Result<Collection<ComponentBatch>> as_batches(
            const archetypes::AnnotationContext& archetype
        );
    };
} // namespace rerun
