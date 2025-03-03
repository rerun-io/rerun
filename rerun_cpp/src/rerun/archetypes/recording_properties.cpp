// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/recording_properties.fbs".

#include "recording_properties.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    RecordingProperties RecordingProperties::clear_fields() {
        auto archetype = RecordingProperties();
        archetype.application_id =
            ComponentBatch::empty<rerun::components::ApplicationId>(Descriptor_application_id)
                .value_or_throw();
        archetype.started =
            ComponentBatch::empty<rerun::components::RecordingStartedTimestamp>(Descriptor_started)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> RecordingProperties::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(3);
        if (application_id.has_value()) {
            columns.push_back(application_id.value().partitioned(lengths_).value_or_throw());
        }
        if (started.has_value()) {
            columns.push_back(started.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(ComponentColumn::from_indicators<RecordingProperties>(
                              static_cast<uint32_t>(lengths_.size())
        )
                              .value_or_throw());
        return columns;
    }

    Collection<ComponentColumn> RecordingProperties::columns() {
        if (application_id.has_value()) {
            return columns(std::vector<uint32_t>(application_id.value().length(), 1));
        }
        if (started.has_value()) {
            return columns(std::vector<uint32_t>(started.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<Collection<ComponentBatch>> AsComponents<archetypes::RecordingProperties>::as_batches(
        const archetypes::RecordingProperties& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        if (archetype.application_id.has_value()) {
            cells.push_back(archetype.application_id.value());
        }
        if (archetype.started.has_value()) {
            cells.push_back(archetype.started.value());
        }
        {
            auto result = ComponentBatch::from_indicator<RecordingProperties>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return rerun::take_ownership(std::move(cells));
    }
} // namespace rerun
