#include "../error_check.hpp"
#include "archetype_test.hpp"

#include <rerun.hpp>
using namespace rerun::archetypes;
using namespace rerun::datatypes;

#define TEST_TAG "[image][archetypes]"

SCENARIO("Image archetype can be created" TEST_TAG) {
    GIVEN("simple 8bit grayscale image") {
        std::vector<uint8_t> data(10 * 10, 0);
        Image reference_image;
        reference_image.buffer = rerun::ComponentBatch::from_loggable(
                                     rerun::components::ImageBuffer(data),
                                     Image::Descriptor_buffer
        )
                                     .value_or_throw();
        reference_image.format =
            rerun::ComponentBatch::from_loggable(
                rerun::components::ImageFormat({10, 10}, ColorModel::L, ChannelDatatype::U8),
                Image::Descriptor_format
            )
                .value_or_throw();

        THEN("no error occurs on image construction from a pointer") {
            auto image_from_ptr =
                check_logged_error([&] { return Image(data.data(), {10, 10}, ColorModel::L); });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, reference_image);
            }
        }
        THEN("no error occurs on image construction from a collection") {
            auto image_from_collection = check_logged_error([&] {
                return Image(rerun::borrow(data), {10, 10}, ColorModel::L);
            });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_collection, reference_image);
            }
        }
        THEN("no error occurs on image construction from the grayscale utility") {
            auto image_from_util =
                check_logged_error([&] { return Image::from_grayscale8(data, {10, 10}); });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_util, reference_image);
            }
        }
    }

    GIVEN("simple 8bit RGB image") {
        std::vector<uint8_t> data(10 * 10 * 3, 0);
        Image reference_image;
        reference_image.buffer = rerun::ComponentBatch::from_loggable(
                                     rerun::components::ImageBuffer(data),
                                     Image::Descriptor_buffer
        )
                                     .value_or_throw();
        reference_image.format =
            rerun::ComponentBatch::from_loggable(
                rerun::components::ImageFormat({10, 10}, ColorModel::RGB, ChannelDatatype::U8),
                Image::Descriptor_format
            )
                .value_or_throw();

        THEN("no error occurs on image construction from a pointer") {
            auto image_from_ptr =
                check_logged_error([&] { return Image(data.data(), {10, 10}, ColorModel::RGB); });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, reference_image);
            }
        }
        THEN("no error occurs on image construction from a collection") {
            auto image_from_collection = check_logged_error([&] {
                return Image(rerun::borrow(data), {10, 10}, ColorModel::RGB);
            });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_collection, reference_image);
            }
        }
        THEN("no error occurs on image construction from the rgb utility") {
            auto image_from_util =
                check_logged_error([&] { return Image::from_rgb24(data, {10, 10}); });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_util, reference_image);
            }
        }
    }

    GIVEN("simple 8bit RGBA image") {
        std::vector<uint8_t> data(10 * 10 * 4, 0);
        Image reference_image;
        reference_image.buffer = rerun::ComponentBatch::from_loggable(
                                     rerun::components::ImageBuffer(data),
                                     Image::Descriptor_buffer
        )
                                     .value_or_throw();
        reference_image.format =
            rerun::ComponentBatch::from_loggable(
                rerun::components::ImageFormat({10, 10}, ColorModel::RGBA, ChannelDatatype::U8),
                Image::Descriptor_format
            )
                .value_or_throw();

        THEN("no error occurs on image construction from a pointer") {
            auto image_from_ptr =
                check_logged_error([&] { return Image(data.data(), {10, 10}, ColorModel::RGBA); });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, reference_image);
            }
        }
        THEN("no error occurs on image construction from a collection") {
            auto image_from_collection = check_logged_error([&] {
                return Image(rerun::borrow(data), {10, 10}, ColorModel::RGBA);
            });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_collection, reference_image);
            }
        }
        THEN("no error occurs on image construction from the rgba utility") {
            auto image_from_util =
                check_logged_error([&] { return Image::from_rgba32(data, {10, 10}); });
            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_util, reference_image);
            }
        }
    }
}
