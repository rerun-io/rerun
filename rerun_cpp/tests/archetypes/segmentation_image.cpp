#include "../error_check.hpp"

#include <rerun/archetypes/segmentation_image.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[segmentation_image][archetypes]"

SCENARIO("segmentation image can be created from tensor data" TEST_TAG) {
    GIVEN("tensor data with correct shape") {
        rerun::datatypes::TensorData data({3, 7}, std::vector<uint8_t>(3 * 7, 0));

        THEN("no error occurs on segmentation image construction") {
            auto segmentation_image =
                check_logged_error([&] { return SegmentationImage(std::move(data)); });

            AND_THEN("width and height got set") {
                CHECK(segmentation_image.data.data.shape[0].name == "height");
                CHECK(segmentation_image.data.data.shape[1].name == "width");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(segmentation_image)>()
                          .serialize(segmentation_image)
                          .is_ok());
            }
        }
    }

    GIVEN("tensor data with correct shape and named dimensions") {
        rerun::datatypes::TensorData data(
            {rerun::datatypes::TensorDimension(3, "rick"),
             rerun::datatypes::TensorDimension(7, "morty")},
            std::vector<uint8_t>(3 * 7, 0)
        );

        THEN("no error occurs on segmentation image construction") {
            auto segmentation_image =
                check_logged_error([&] { return SegmentationImage(std::move(data)); });

            AND_THEN("tensor dimensions are unchanged") {
                CHECK(segmentation_image.data.data.shape[0].name == "rick");
                CHECK(segmentation_image.data.data.shape[1].name == "morty");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(segmentation_image)>()
                          .serialize(segmentation_image)
                          .is_ok());
            }
        }
    }

    GIVEN("tensor data with too high rank") {
        rerun::datatypes::TensorData data(
            {
                {
                    // (🎶 Sie sind geheimnisvoll, doch sie sind supertoll 🎶)
                    rerun::datatypes::TensorDimension(1, "tick"),
                    rerun::datatypes::TensorDimension(2, "trick"),
                    rerun::datatypes::TensorDimension(3, "track"),
                },
            },
            std::vector<uint8_t>(1 * 2 * 3, 0)
        );

        THEN("a warning occurs on segmentation image construction") {
            auto segmentation_image = check_logged_error(
                [&] { return SegmentationImage(std::move(data)); },
                rerun::ErrorCode::InvalidTensorDimension
            );

            AND_THEN("tensor dimension names are unchanged") {
                CHECK(segmentation_image.data.data.shape[0].name == "tick");
                CHECK(segmentation_image.data.data.shape[1].name == "trick");
                CHECK(segmentation_image.data.data.shape[2].name == "track");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(segmentation_image)>()
                          .serialize(segmentation_image)
                          .is_ok());
            }
        }
    }

    GIVEN("tensor data with too low rank") {
        rerun::datatypes::TensorData data(
            {{
                rerun::datatypes::TensorDimension(1, "dr robotnik"),
            }},
            std::vector<uint8_t>(1, 0)
        );

        THEN("a warning occurs on segmentation image construction") {
            auto segmentation_image = check_logged_error(
                [&] { return SegmentationImage(std::move(data)); },
                rerun::ErrorCode::InvalidTensorDimension
            );

            AND_THEN("tensor dimension names are unchanged") {
                CHECK(segmentation_image.data.data.shape[0].name == "dr robotnik");
            }

            AND_THEN("serialization succeeds") {
                CHECK(rerun::AsComponents<decltype(segmentation_image)>()
                          .serialize(segmentation_image)
                          .is_ok());
            }
        }
    }
}
