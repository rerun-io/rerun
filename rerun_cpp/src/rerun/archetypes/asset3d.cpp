// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/asset3d.fbs".

#include "asset3d.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char Asset3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Asset3DIndicator";

        AnonymousComponentBatch Asset3D::indicator() {
            return ComponentBatch<
                components::IndicatorComponent<Asset3D::INDICATOR_COMPONENT_NAME>>(nullptr, 1);
        }

        std::vector<AnonymousComponentBatch> Asset3D::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(3);

            comp_batches.emplace_back(data);
            if (media_type.has_value()) {
                comp_batches.emplace_back(media_type.value());
            }
            if (transform.has_value()) {
                comp_batches.emplace_back(transform.value());
            }
            comp_batches.emplace_back(Asset3D::indicator());

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
