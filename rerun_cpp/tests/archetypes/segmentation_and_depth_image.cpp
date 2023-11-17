#include "../error_check.hpp"

#include <rerun/archetypes/depth_image.hpp>
#include <rerun/archetypes/segmentation_image.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[image][archetypes]"

template <typename ImageType>
void run_image_tests() {
    GIVEN("tensor data with correct shape") {
        rerun::datatypes::TensorData data({3, 7}, std::vector<uint8_t>(3 * 7, 0));
        THEN("no error occurs on image construction") {
            auto image = check_logged_error([&] { return ImageType(std::move(data)); });

            AND_THEN("width and height got set") {
                CHECK(image.data.data.shape[0].name == "height");
                CHECK(image.data.data.shape[1].name == "width");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(image)>().serialize(image).is_ok());
            }
        }
    }

    GIVEN("tensor data with correct shape and named dimensions") {
        rerun::datatypes::TensorData data(
            {rerun::datatypes::TensorDimension(3, "rick"),
             rerun::datatypes::TensorDimension(7, "morty")},
            std::vector<uint8_t>(3 * 7, 0)
        );
        THEN("no error occurs on image construction") {
            auto image = check_logged_error([&] { return ImageType(std::move(data)); });

            AND_THEN("tensor dimensions are unchanged") {
                CHECK(image.data.data.shape[0].name == "rick");
                CHECK(image.data.data.shape[1].name == "morty");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(image)>().serialize(image).is_ok());
            }
        }
    }

    GIVEN("tensor data with too high rank") {
        rerun::datatypes::TensorData data(
            {
                {
                    rerun::datatypes::TensorDimension(1, "tick"),
                    rerun::datatypes::TensorDimension(2, "trick"),
                    rerun::datatypes::TensorDimension(3, "track"),
                },
            },
            std::vector<uint8_t>(1 * 2 * 3, 0)
        );
        THEN("a warning occurs on image construction") {
            auto image = check_logged_error(
                [&] { return ImageType(std::move(data)); },
                rerun::ErrorCode::InvalidTensorDimension
            );

            AND_THEN("tensor dimension names are unchanged") {
                CHECK(image.data.data.shape[0].name == "tick");
                CHECK(image.data.data.shape[1].name == "trick");
                CHECK(image.data.data.shape[2].name == "track");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(image)>().serialize(image).is_ok());
            }
        }
    }

    GIVEN("tensor data with too low rank") {
        rerun::datatypes::TensorData data(
            {
                rerun::datatypes::TensorDimension(1, "dr robotnik"),
            },
            std::vector<uint8_t>(1, 0)
        );
        THEN("a warning occurs on image construction") {
            auto image = check_logged_error(
                [&] { return ImageType(std::move(data)); },
                rerun::ErrorCode::InvalidTensorDimension
            );

            AND_THEN("tensor dimension names are unchanged") {
                CHECK(image.data.data.shape[0].name == "dr robotnik");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(image)>().serialize(image).is_ok());
            }
        }
    }
}

SCENARIO("Segmentation archetype image can be created from tensor data." TEST_TAG) {
    run_image_tests<SegmentationImage>();
}

SCENARIO("Depth archetype image can be created from tensor data." TEST_TAG) {
    run_image_tests<DepthImage>();
}
