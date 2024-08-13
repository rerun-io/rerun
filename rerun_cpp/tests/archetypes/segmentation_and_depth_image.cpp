#include "../error_check.hpp"
#include "archetype_test.hpp"

#include <rerun/archetypes/depth_image.hpp>
#include <rerun/archetypes/segmentation_image.hpp>

using namespace rerun::archetypes;
using namespace rerun::datatypes;

#define TEST_TAG "[image][archetypes]"

template <typename ImageType>
void run_image_tests() {
    GIVEN("a vector of u8 data") {
        std::vector<uint8_t> data(10 * 10, 0);
        ImageType reference_image;
        reference_image.buffer = rerun::borrow(data);
        reference_image.format = ImageFormat({10, 10}, ChannelDatatype::U8);
        INFO("Format byte size: " << reference_image.format.image_format.num_bytes());

        THEN("no error occurs on image construction from a pointer") {
            auto image_from_ptr = check_logged_error([&] {
                return ImageType(data.data(), {10, 10});
            });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, reference_image);
            }
        }
        THEN("no error occurs on image construction from a collection") {
            auto image_from_collection = check_logged_error([&] {
                return ImageType(rerun::borrow(data), {10, 10});
            });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_collection, reference_image);
            }
        }

        THEN("no error occurs on image construction from an untyped pointer") {
            const void* ptr = reinterpret_cast<const void*>(data.data());
            auto image_from_ptr = check_logged_error([&] {
                return ImageType(ptr, {10, 10}, ChannelDatatype::U8);
            });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, reference_image);
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
