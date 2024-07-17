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

template <typename ImageType>
void run_tensor_image_tests() {
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

    GIVEN("tensor data with too high dimensionality") {
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

    GIVEN("tensor data with too low dimensionality") {
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

    GIVEN("a vector of data") {
        std::vector<uint8_t> data(10 * 10, 0);
        THEN("no error occurs on image construction with either the vector or a data pointer") {
            auto image_from_vector = check_logged_error([&] { return ImageType({10, 10}, data); });
            auto image_from_ptr = check_logged_error([&] {
                return ImageType({10, 10}, data.data());
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
    run_tensor_image_tests<SegmentationImage>();
}
