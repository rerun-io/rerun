#pragma once

#include "collection_adapter.hpp"
#include "datatypes/class_description_map_elem.hpp"

namespace rerun {
    template <>
    struct CollectionAdapter<
        datatypes::ClassDescriptionMapElem, Collection<datatypes::ClassDescription>> {
        Collection<datatypes::ClassDescriptionMapElem> operator()(
            const Collection<datatypes::ClassDescription>& class_descriptions
        ) {
            std::vector<datatypes::ClassDescriptionMapElem> class_map;
            class_map.reserve(class_descriptions.size());
            for (const auto& class_description : class_descriptions) {
                class_map.emplace_back(std::move(class_description));
            }
            return Collection<datatypes::ClassDescriptionMapElem>::take_ownership(
                std::move(class_map)
            );
        }
    };
} // namespace rerun
