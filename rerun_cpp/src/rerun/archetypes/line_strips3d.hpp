// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips3d.fbs".

#pragma once

#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/instance_key.hpp"
#include "../components/line_strip3d.hpp"
#include "../components/radius.hpp"
#include "../components/text.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"
#include "../util.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: 3D line strips with positions and optional colors, radii, labels, etc.
        ///
        /// ## Example
        ///
        /// ### Many strips
        /// ```cpp,ignore
        /// #include <rerun.hpp>
        ///
        /// int main() {
        ///     auto rec = rerun::RecordingStream("rerun_example_line_strip3d");
        ///     rec.spawn().throw_on_failure();
        ///
        ///     std::vector<rerun::datatypes::Vec3D> strip1 = {
        ///         {0.f, 0.f, 2.f},
        ///         {1.f, 0.f, 2.f},
        ///         {1.f, 1.f, 2.f},
        ///         {0.f, 1.f, 2.f},
        ///     };
        ///     std::vector<rerun::datatypes::Vec3D> strip2 = {
        ///         {0.f, 0.f, 0.f},
        ///         {0.f, 0.f, 1.f},
        ///         {1.f, 0.f, 0.f},
        ///         {1.f, 0.f, 1.f},
        ///         {1.f, 1.f, 0.f},
        ///         {1.f, 1.f, 1.f},
        ///         {0.f, 1.f, 0.f},
        ///         {0.f, 1.f, 1.f},
        ///     };
        ///     rec.log(
        ///         "strips",
        ///         rerun::LineStrips3D({strip1, strip2})
        ///             .with_colors({0xFF0000FF, 0x00FF00FF})
        ///             .with_radii({0.025f, 0.005f})
        ///             .with_labels({"one strip here", "and one strip there"})
        ///     );
        /// }
        /// ```
        struct LineStrips3D {
            /// All the actual 3D line strips that make up the batch.
            ComponentBatch<rerun::components::LineStrip3D> strips;

            /// Optional radii for the line strips.
            std::optional<ComponentBatch<rerun::components::Radius>> radii;

            /// Optional colors for the line strips.
            std::optional<ComponentBatch<rerun::components::Color>> colors;

            /// Optional text labels for the line strips.
            std::optional<ComponentBatch<rerun::components::Text>> labels;

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            std::optional<ComponentBatch<rerun::components::ClassId>> class_ids;

            /// Unique identifiers for each individual line strip in the batch.
            std::optional<ComponentBatch<rerun::components::InstanceKey>> instance_keys;

            /// Name of the indicator component, used to identify the archetype when converting to a list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            /// Indicator component, used to identify the archetype when converting to a list of components.
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            LineStrips3D() = default;
            LineStrips3D(LineStrips3D&& other) = default;

            explicit LineStrips3D(ComponentBatch<rerun::components::LineStrip3D> _strips)
                : strips(std::move(_strips)) {}

            /// Optional radii for the line strips.
            LineStrips3D with_radii(ComponentBatch<rerun::components::Radius> _radii) && {
                radii = std::move(_radii);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Optional colors for the line strips.
            LineStrips3D with_colors(ComponentBatch<rerun::components::Color> _colors) && {
                colors = std::move(_colors);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Optional text labels for the line strips.
            LineStrips3D with_labels(ComponentBatch<rerun::components::Text> _labels) && {
                labels = std::move(_labels);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Optional `ClassId`s for the lines.
            ///
            /// The class ID provides colors and labels if not specified explicitly.
            LineStrips3D with_class_ids(ComponentBatch<rerun::components::ClassId> _class_ids) && {
                class_ids = std::move(_class_ids);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Unique identifiers for each individual line strip in the batch.
            LineStrips3D with_instance_keys(
                ComponentBatch<rerun::components::InstanceKey> _instance_keys
            ) && {
                instance_keys = std::move(_instance_keys);
                // See: https://github.com/rerun-io/rerun/issues/4027
                WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return strips.size();
            }
        };

    } // namespace archetypes

    template <typename T>
    struct AsComponents;

    template <>
    struct AsComponents<archetypes::LineStrips3D> {
        /// Serialize all set component batches.
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const archetypes::LineStrips3D& archetype
        );
    };
} // namespace rerun
