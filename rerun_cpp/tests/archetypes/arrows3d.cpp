#include "archetype_test.hpp"

#include <rerun/archetypes/arrows3d.hpp>

using namespace rerun::archetypes;

#define TEST_TAG "[arrow3d][archetypes]"

SCENARIO(
    "Arrows3D archetype can be serialized with the same result for manually built instances and "
    "the builder pattern",
    TEST_TAG
) {
    GIVEN("Constructed from builder and manually") {
        auto from_builder = Arrows3D::from_vectors({{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}})
                                .with_origins({{4.0, 5.0, 6.0}, {40.0, 50.0, 60.0}})
                                .with_radii({1.0, 10.0})
                                .with_colors({{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}})
                                .with_labels({"hello", "friend"})
                                .with_class_ids({126, 127});

        Arrows3D from_manual;
        from_manual.vectors = rerun::ComponentBatch::from_loggable<rerun::components::Vector3D>(
                                  {{1.0, 2.0, 3.0}, {10.0, 20.0, 30.0}},
                                  Arrows3D::Descriptor_vectors
        )
                                  .value_or_throw();
        from_manual.origins = rerun::ComponentBatch::from_loggable<rerun::components::Position3D>(
                                  {{4.0, 5.0, 6.0}, {40.0, 50.0, 60.0}},
                                  Arrows3D::Descriptor_origins
        )
                                  .value_or_throw();
        from_manual.radii = rerun::ComponentBatch::from_loggable<rerun::components::Radius>(
                                {1.0, 10.0},
                                Arrows3D::Descriptor_radii
        )
                                .value_or_throw();
        from_manual.colors = rerun::ComponentBatch::from_loggable<rerun::components::Color>(
                                 {{0xAA, 0x00, 0x00, 0xCC}, {0x00, 0xBB, 0x00, 0xDD}},
                                 Arrows3D::Descriptor_colors
        )
                                 .value_or_throw();
        from_manual.labels = rerun::ComponentBatch::from_loggable<rerun::components::Text>(
                                 {"hello", "friend"},
                                 Arrows3D::Descriptor_labels
        )
                                 .value_or_throw();
        from_manual.class_ids = rerun::ComponentBatch::from_loggable<rerun::components::ClassId>(
                                    {126, 127},
                                    Arrows3D::Descriptor_class_ids
        )
                                    .value_or_throw();

        test_compare_archetype_serialization(from_manual, from_builder);
    }
}
