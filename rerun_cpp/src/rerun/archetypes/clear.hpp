// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/clear.fbs".

#pragma once

#include "../component_batch.hpp"
#include "../components/clear_is_recursive.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: Empties all the components of an entity.
        ///
        /// ## Example
        ///
        /// ### Flat
        /// ```cpp,ignore
        /// // Log a batch of 3D arrows.
        ///
        /// #include <rerun.hpp>
        ///
        /// #include <cmath>
        /// #include <numeric>
        ///
        /// int main() {
        ///     auto rec = rerun::RecordingStream("rerun_example_clear_simple");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     std::vector<rerun::components::Vector3D> vectors = {
        ///         {1.0, 0.0, 0.0},
        ///         {0.0, -1.0, 0.0},
        ///         {-1.0, 0.0, 0.0},
        ///         {0.0, 1.0, 0.0},
        ///     };
        ///     std::vector<rerun::components::Position3D> origins = {
        ///         {-0.5, 0.5, 0.0},
        ///         {0.5, 0.5, 0.0},
        ///         {0.5, -0.5, 0.0},
        ///         {-0.5, -0.5, 0.0},
        ///     };
        ///     std::vector<rerun::components::Color> colors = {
        ///         {200, 0, 0},
        ///         {0, 200, 0},
        ///         {0, 0, 200},
        ///         {200, 0, 200},
        ///     };
        ///
        ///     // Log a handful of arrows.
        ///     for (size_t i = 0; i <vectors.size(); ++i) {
        ///         auto entity_path = "arrows/" + std::to_string(i);
        ///         rec.log(
        ///             entity_path.c_str(),
        ///             rerun::Arrows3D::from_vectors(vectors[i])
        ///                 .with_origins(origins[i])
        ///                 .with_colors(colors[i])
        ///         );
        ///     }
        ///
        ///     // Now clear them, one by one on each tick.
        ///     for (size_t i = 0; i <vectors.size(); ++i) {
        ///         auto entity_path = "arrows/" + std::to_string(i);
        ///         rec.log(entity_path.c_str(), rerun::Clear::FLAT);
        ///     }
        /// }
        /// ```
        struct Clear {
            rerun::components::ClearIsRecursive is_recursive;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            /// Indicator component, used to identify the archetype when converting to a list of
            /// components.
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            // Extensions to generated type defined in 'clear_ext.cpp'

            static const Clear FLAT;

            static const Clear RECURSIVE;

            Clear(bool _is_recursive = false)
                : Clear(components::ClearIsRecursive(_is_recursive)) {}

          public:
            Clear() = default;
            Clear(Clear&& other) = default;

            explicit Clear(rerun::components::ClearIsRecursive _is_recursive)
                : is_recursive(std::move(_is_recursive)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }
        };

    } // namespace archetypes

    template <typename T>
    struct AsComponents;

    template <>
    struct AsComponents<archetypes::Clear> {
        /// Serialize all set component batches.
        static Result<std::vector<SerializedComponentBatch>> serialize(
            const archetypes::Clear& archetype
        );
    };
} // namespace rerun
