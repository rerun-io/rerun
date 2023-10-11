#include <catch2/catch_test_macros.hpp>

#include <rerun/archetypes/points2d.hpp>
#include <rerun/component_batch.hpp>
#include <rerun/components/position2d.hpp>

#include "archetypes/archetype_test.hpp"

#define TEST_TAG "[component_batch]"

SCENARIO("ComponentBatch creation via common adaptors", TEST_TAG) {
    GIVEN("a vector of components") {
        std::vector<rerun::components::Position2D> components = {
            rerun::components::Position2D(0.0f, 1.0f),
            rerun::components::Position2D(1.0f, 2.0f),
        };

        THEN("a component batch created from it borrows its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(components);
            CHECK(batch.size() == components.size());
            CHECK(batch.get_ownership() == rerun::BatchOwnership::Borrowed);
        }
        THEN("a component batch created from it moving it owns the data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(std::move(components));
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }
    // todo: add explicit move test
    GIVEN("a temporary vector of components") {
        THEN("a component batch created from it owns its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(
                std::vector<rerun::components::Position2D>{
                    rerun::components::Position2D(0.0f, 1.0f),
                    rerun::components::Position2D(1.0f, 2.0f),
                }
            );
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }

    GIVEN("an std::array of components") {
        std::array<rerun::components::Position2D, 2> components = {
            rerun::components::Position2D(0.0f, 1.0f),
            rerun::components::Position2D(1.0f, 2.0f),
        };

        THEN("a component batch created from it borrows its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(components);
            CHECK(batch.size() == components.size());
            CHECK(batch.get_ownership() == rerun::BatchOwnership::Borrowed);
        }
        THEN("a component batch created from it moving it owns the data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(std::move(components));
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }
    GIVEN("a temporary std::array of components") {
        THEN("a component batch created from it owns its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(
                std::array<rerun::components::Position2D, 2>{
                    rerun::components::Position2D(0.0f, 1.0f),
                    rerun::components::Position2D(1.0f, 2.0f),
                }
            );
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }

    GIVEN("a c-array of components") {
        rerun::components::Position2D components[] = {
            rerun::components::Position2D(0.0f, 1.0f),
            rerun::components::Position2D(1.0f, 2.0f),
        };

        THEN("a component batch created from it borrows its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(components);
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::Borrowed);
        }
        THEN("a component batch created from moving it owns the data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(std::move(components));
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }

    GIVEN("a single components") {
        rerun::components::Position2D component = rerun::components::Position2D(0.0f, 1.0f);

        THEN("a component batch created from it borrows its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(component);
            CHECK(batch.size() == 1);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::Borrowed);
        }
        THEN("a component batch created from it moving it owns the data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(std::move(component));
            CHECK(batch.size() == 1);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }
    GIVEN("a single temporary component") {
        THEN("a component batch created from it borrows its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(
                rerun::components::Position2D(0.0f, 1.0f)
            );
            CHECK(batch.size() == 1);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::VectorOwned);
        }
    }
}

struct MyVec2Container {
    std::vector<float> vecs;
};

namespace rerun {
    template <>
    struct ComponentBatchAdapter<components::Position2D, MyVec2Container> {
        ComponentBatch<components::Position2D> operator()(const MyVec2Container& container) {
            // Sanity check that this is binary compatible.
            static_assert(sizeof(components::Position2D) == sizeof(float) * 2);
            static_assert(alignof(components::Position2D) <= sizeof(float));

            return ComponentBatch<components::Position2D>::borrow(
                reinterpret_cast<const components::Position2D*>(container.vecs.data()),
                container.vecs.size() / 2
            );
        }

        // TODO: fill in rvalue version and document when it's needed (almost always!)
    };
} // namespace rerun

SCENARIO(
    "ComponentBatch creation via a custom adapter for a datalayout compatible type", TEST_TAG
) {
    GIVEN("A custom vec2 container with a defined adapter") {
        MyVec2Container container;
        container.vecs = {0.0f, 1.0f, 2.0f, 3.0f};

        THEN("a component batch created from it that its data") {
            const rerun::ComponentBatch<rerun::components::Position2D> batch(container);
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::BatchOwnership::Borrowed);
        }
        THEN("A Point2D archetype can be directly created from this container") {
            const rerun::archetypes::Points2D from_custom_container(container);

            CHECK(from_custom_container.positions.size() == 2);
            CHECK(
                from_custom_container.positions.get_ownership() == rerun::BatchOwnership::Borrowed
            );

            AND_THEN("it can be serialized and is identical to creation from rerun types directly"
            ) {
                const rerun::archetypes::Points2D from_rerun_vector({{0.0f, 1.0f}, {2.0f, 3.0f}});

                CHECK(from_rerun_vector.positions.size() == 2);
                CHECK(
                    from_rerun_vector.positions.get_ownership() ==
                    rerun::BatchOwnership::VectorOwned
                );

                test_compare_archetype_serialization(from_custom_container, from_rerun_vector);
            }
        }
    }
}
