#include "../error_check.hpp"
#include "archetype_test.hpp"

#include <rerun/archetypes/depth_image.hpp>
#include <rerun/archetypes/segmentation_image.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[image][archetypes]"

template <typename ImageType>
void run_image_tests() {
    GIVEN("a vector of data") {
        std::vector<uint8_t> data(10 * 10, 0);
        THEN("no error occurs on image construction with either the vector or a data pointer") {
            auto image_from_vector = check_logged_error([&] { return ImageType(data, {10, 10}); });
            auto image_from_ptr = check_logged_error([&] {
                return ImageType(data.data(), {10, 10});
            });

            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, image_from_vector);
            }
        }
    }
}

SCENARIO("Depth archetype image can be created." TEST_TAG) {
    run_image_tests<DepthImage>();
}

SCENARIO("Segmentation archetype image can be created from tensor data." TEST_TAG) {
    run_image_tests<SegmentationImage>();
}
