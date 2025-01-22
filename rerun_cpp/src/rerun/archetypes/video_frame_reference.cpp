// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/video_frame_reference.fbs".

#include "video_frame_reference.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    VideoFrameReference VideoFrameReference::clear_fields() {
        auto archetype = VideoFrameReference();
        archetype.timestamp =
            ComponentBatch::empty<rerun::components::VideoTimestamp>(Descriptor_timestamp)
                .value_or_throw();
        archetype.video_reference =
            ComponentBatch::empty<rerun::components::EntityPath>(Descriptor_video_reference)
                .value_or_throw();
        return archetype;
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::VideoFrameReference>::serialize(
        const archetypes::VideoFrameReference& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        if (archetype.timestamp.has_value()) {
            cells.push_back(archetype.timestamp.value());
        }
        if (archetype.video_reference.has_value()) {
            cells.push_back(archetype.video_reference.value());
        }
        {
            auto indicator = VideoFrameReference::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
