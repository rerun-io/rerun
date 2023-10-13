#include "archetype_test.hpp"

#include <rerun/archetypes/annotation_context.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[annotation_context][archetypes]"

SCENARIO(
    "AnnotationContext archetype's class descriptions can be constructed in various ways and "
    "serialized",
    TEST_TAG
) {
    GIVEN("A annotation context created with various utilities and one manual step by step") {
        rerun::archetypes::AnnotationContext from_utilities({
            rerun::datatypes::ClassDescription({1, "hello"}),
            rerun::datatypes::ClassDescription(rerun::datatypes::AnnotationInfo(1, "hello")),
            rerun::datatypes::ClassDescription(
                {2, "world", rerun::datatypes::Rgba32(3, 4, 5)},
                {{17, "head"}, {42, "shoulders"}},
                {
                    {1, 2},
                    {3, 4},
                }
            ),
            rerun::datatypes::ClassDescription(
                rerun::datatypes::AnnotationInfo(2, "world", rerun::datatypes::Rgba32(3, 4, 5)),
                {
                    rerun::datatypes::AnnotationInfo(17, "head"),
                    rerun::datatypes::AnnotationInfo(42, "shoulders"),
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
            rerun::datatypes::ClassDescriptionMapElem element;
            rerun::datatypes::KeypointPair pair;
            rerun::datatypes::AnnotationInfo keypoint_annotation;

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
                element.class_description.info.color = rerun::datatypes::Rgba32(3, 4, 5);
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
