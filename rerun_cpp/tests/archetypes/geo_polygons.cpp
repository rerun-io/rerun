#include "archetype_test.hpp"

#include <arrow/array/array_nested.h>
#include <arrow/array/array_primitive.h>
#include <rerun/archetypes/geo_polygons.hpp>

#include <cstdint>
#include <vector>

using rerun::archetypes::GeoPolygons;
using rerun::components::GeoPolygon;

#define TEST_TAG "[geopolygons][archetypes]"

TEST_CASE(
    "GeoPolygon serializes different ring counts and preserves latitude/longitude", TEST_TAG
) {
    const std::vector<GeoPolygon> polygons = {
        GeoPolygon::from_lat_lon({
            {{10.0, 20.0}, {10.0, 24.0}, {14.0, 24.0}, {14.0, 20.0}},
            {{11.0, 21.0}, {11.0, 22.0}, {12.0, 21.0}},
        }),
        GeoPolygon::from_lat_lon({{{30.0, 40.0}, {30.0, 41.0}, {31.0, 40.0}}}),
    };
    const auto result = rerun::Loggable<GeoPolygon>::to_arrow(polygons.data(), polygons.size());
    REQUIRE(result.is_ok());
    REQUIRE(result.value->ValidateFull().ok());
    const auto array = std::static_pointer_cast<arrow::StructArray>(result.value);
    REQUIRE(array->length() == 2);
    REQUIRE(array->num_fields() == 2);

    const auto positions =
        std::static_pointer_cast<arrow::ListArray>(array->GetFieldByName("lat_lon"));
    const auto lengths =
        std::static_pointer_cast<arrow::ListArray>(array->GetFieldByName("ring_lengths"));
    REQUIRE(positions != nullptr);
    REQUIRE(lengths != nullptr);
    CHECK(positions->value_offset(0) == 0);
    CHECK(positions->value_offset(1) == 7);
    CHECK(positions->value_offset(2) == 10);
    CHECK(lengths->value_offset(0) == 0);
    CHECK(lengths->value_offset(1) == 2);
    CHECK(lengths->value_offset(2) == 3);

    const auto ring_lengths = std::static_pointer_cast<arrow::UInt64Array>(lengths->values());
    REQUIRE(ring_lengths->length() == 3);
    CHECK(ring_lengths->Value(0) == 4);
    CHECK(ring_lengths->Value(1) == 3);
    CHECK(ring_lengths->Value(2) == 3);

    const auto pairs = std::static_pointer_cast<arrow::FixedSizeListArray>(positions->values());
    const auto coordinates = std::static_pointer_cast<arrow::DoubleArray>(pairs->values());
    const std::vector<double> expected = {
        10.0, 20.0, 10.0, 24.0, 14.0, 24.0, 14.0, 20.0, 11.0, 21.0,
        11.0, 22.0, 12.0, 21.0, 30.0, 40.0, 30.0, 41.0, 31.0, 40.0,
    };
    REQUIRE(coordinates->length() == static_cast<int64_t>(expected.size()));
    for (size_t index = 0; index < expected.size(); ++index) {
        INFO("Coordinate " << index);
        CHECK(coordinates->Value(static_cast<int64_t>(index)) == expected[index]);
    }
}

TEST_CASE("GeoPolygons empty batches and colors-only updates remain distinct", TEST_TAG) {
    const auto empty = GeoPolygons(std::vector<GeoPolygon>{});
    REQUIRE(empty.polygons.has_value());
    REQUIRE(empty.polygons->array->ValidateFull().ok());
    CHECK(empty.polygons->array->length() == 0);

    const auto color_update =
        GeoPolygons::update_fields().with_colors({rerun::components::Color(10, 20, 30, 128)});
    CHECK_FALSE(color_update.polygons.has_value());
    REQUIRE(color_update.colors.has_value());
    REQUIRE(color_update.colors->array->ValidateFull().ok());
    CHECK(color_update.colors->array->length() == 1);
    const auto colors = std::static_pointer_cast<arrow::UInt32Array>(color_update.colors->array);
    CHECK(colors->Value(0) == 0x0A141E80);
    REQUIRE(rerun::AsComponents<GeoPolygons>::as_batches(color_update).is_ok());
}
