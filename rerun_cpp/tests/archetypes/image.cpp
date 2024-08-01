#include "../error_check.hpp"
#include "archetype_test.hpp"

#include <rerun.hpp>
using namespace rerun::archetypes;
using namespace rerun::components;

#define TEST_TAG "[image][archetypes]"

SCENARIO("Image archetype can be created" TEST_TAG) {
    GIVEN("Image::from_elements") {
        std::vector<uint8_t> data(10 * 10, 0);
        THEN("no error occurs on image construction with either the vector or a data pointer") {
            auto image_from_vector = check_logged_error([&] {
                return Image::from_elements({10, 10}, ColorModel::L, data);
            });
            auto image_from_ptr = check_logged_error([&] {
                return Image::from_color_model_and_bytes(
                    {10, 10},
                    ColorModel::L,
                    ChannelDatatype::U8,
                    data
                );
            });

            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, image_from_vector);
            }
        }
    }
}
