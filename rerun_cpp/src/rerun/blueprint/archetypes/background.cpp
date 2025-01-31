// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/background.fbs".

#include "background.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    Background Background::clear_fields() {
        auto archetype = Background();
        archetype.kind =
            ComponentBatch::empty<rerun::blueprint::components::BackgroundKind>(Descriptor_kind)
                .value_or_throw();
        archetype.color =
            ComponentBatch::empty<rerun::components::Color>(Descriptor_color).value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> Background::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(3);
        if (kind.has_value()) {
            columns.push_back(kind.value().partitioned(lengths_).value_or_throw());
        }
        if (color.has_value()) {
            columns.push_back(color.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<Background>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> Background::columns() {
        if (kind.has_value()) {
            return columns(std::vector<uint32_t>(kind.value().length(), 1));
        }
        if (color.has_value()) {
            return columns(std::vector<uint32_t>(color.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<blueprint::archetypes::Background>::serialize(
        const blueprint::archetypes::Background& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        if (archetype.kind.has_value()) {
            cells.push_back(archetype.kind.value());
        }
        if (archetype.color.has_value()) {
            cells.push_back(archetype.color.value());
        }
        {
            auto result = ComponentBatch::from_indicator<Background>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
