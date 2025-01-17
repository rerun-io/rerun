// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_slice_selection.fbs".

#pragma once

#include "../../blueprint/components/tensor_dimension_index_slider.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../component_batch.hpp"
#include "../../components/tensor_dimension_index_selection.hpp"
#include "../../components/tensor_height_dimension.hpp"
#include "../../components/tensor_width_dimension.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Specifies a 2D slice of a tensor.
    struct TensorSliceSelection {
        /// Which dimension to map to width.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        std::optional<rerun::components::TensorWidthDimension> width;

        /// Which dimension to map to height.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        std::optional<rerun::components::TensorHeightDimension> height;

        /// Selected indices for all other dimensions.
        ///
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        std::optional<Collection<rerun::components::TensorDimensionIndexSelection>> indices;

        /// Any dimension listed here will have a slider for the index.
        ///
        /// Edits to the sliders will directly manipulate dimensions on the `indices` list.
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        /// If not specified, adds slides for any dimension in `indices`.
        std::optional<Collection<rerun::blueprint::components::TensorDimensionIndexSlider>> slider;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.TensorSliceSelectionIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] =
            "rerun.blueprint.archetypes.TensorSliceSelection";

      public:
        TensorSliceSelection() = default;
        TensorSliceSelection(TensorSliceSelection&& other) = default;
        TensorSliceSelection(const TensorSliceSelection& other) = default;
        TensorSliceSelection& operator=(const TensorSliceSelection& other) = default;
        TensorSliceSelection& operator=(TensorSliceSelection&& other) = default;

        /// Which dimension to map to width.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        TensorSliceSelection with_width(rerun::components::TensorWidthDimension _width) && {
            width = std::move(_width);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Which dimension to map to height.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        TensorSliceSelection with_height(rerun::components::TensorHeightDimension _height) && {
            height = std::move(_height);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Selected indices for all other dimensions.
        ///
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        TensorSliceSelection with_indices(
            Collection<rerun::components::TensorDimensionIndexSelection> _indices
        ) && {
            indices = std::move(_indices);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Any dimension listed here will have a slider for the index.
        ///
        /// Edits to the sliders will directly manipulate dimensions on the `indices` list.
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        /// If not specified, adds slides for any dimension in `indices`.
        TensorSliceSelection with_slider(
            Collection<rerun::blueprint::components::TensorDimensionIndexSlider> _slider
        ) && {
            slider = std::move(_slider);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::TensorSliceSelection> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::TensorSliceSelection& archetype
        );
    };
} // namespace rerun
