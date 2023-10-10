#include "archetype_test.hpp"

#include <rerun/archetypes/annotation_context.hpp>

namespace rr = rerun;
using namespace rr::archetypes;

#define TEST_TAG "[annotation_context][archetypes]"

SCENARIO(
    "AnnotationContext archetype's class descriptions can be constructed in various ways and "
    "serialized",
    TEST_TAG
) {
    GIVEN("A annotation context created with various utilities and one manual step by step") {
        rr::archetypes::AnnotationContext from_utilities({
            rr::datatypes::ClassDescription({1, "hello"}),
            rr::datatypes::ClassDescription(rr::datatypes::AnnotationInfo(1, "hello")),
            rr::datatypes::ClassDescription(
                {2, "world", rr::datatypes::Rgba32(3, 4, 5)},
                {{17, "head"}, {42, "shoulders"}},
                {
                    {1, 2},
                    {3, 4},
                }
            ),
            rr::datatypes::ClassDescription(
                rr::datatypes::AnnotationInfo(2, "world", rr::datatypes::Rgba32(3, 4, 5)),
                {
                    rr::datatypes::AnnotationInfo(17, "head"),
                    rr::datatypes::AnnotationInfo(42, "shoulders"),
                },
                {
                    std::pair<uint16_t, uint16_t>(1, 2),
                    std::pair<uint16_t, uint16_t>(3, 4),
                }
            ),
        });

        AnnotationContext manual_archetype;
        auto& class_map = manual_archetype.context.class_map;
        {
            rr::datatypes::ClassDescriptionMapElem element;
            rr::datatypes::KeypointPair pair;
            rr::datatypes::AnnotationInfo keypoint_annotation;

            {
                element.class_id = 1;
                element.class_description.info.id = 1;
                element.class_description.info.color = std::nullopt;
                element.class_description.info.label = "hello";
                class_map.push_back(element);
                class_map.push_back(element);
            }
            {
                element.class_id = 2;
                element.class_description.info.id = 2;
                element.class_description.info.color = rr::datatypes::Rgba32(3, 4, 5);
                element.class_description.info.label = "world";

                keypoint_annotation.id = 17;
                keypoint_annotation.color = std::nullopt;
                keypoint_annotation.label = "head";
                element.class_description.keypoint_annotations.push_back(keypoint_annotation);

                keypoint_annotation.id = 42;
                keypoint_annotation.color = std::nullopt;
                keypoint_annotation.label = "shoulders";
                element.class_description.keypoint_annotations.push_back(keypoint_annotation);

                pair.keypoint0 = 1;
                pair.keypoint1 = 2;
                element.class_description.keypoint_connections.push_back(pair);

                pair.keypoint0 = 3;
                pair.keypoint1 = 4;
                element.class_description.keypoint_connections.push_back(pair);

                class_map.push_back(element);
                class_map.push_back(element);
            }
        }

        test_compare_archetype_serialization(from_utilities, manual_archetype);
    }
}
