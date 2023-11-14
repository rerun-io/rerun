#include <catch2/catch_test_macros.hpp>

#include <rerun/archetypes/points2d.hpp>
#include <rerun/collection.hpp>
#include <rerun/collection_adapter_builtins.hpp>
#include <rerun/components/position2d.hpp>

#include "archetypes/archetype_test.hpp"

#define TEST_TAG "[collection]"

using namespace rerun::components;

SCENARIO("Collection creation via common adapters", TEST_TAG) {
    GIVEN("a vector of components") {
        std::vector<Position2D> components = {
            Position2D(0.0f, 1.0f),
            Position2D(1.0f, 2.0f),
        };

        THEN("a collection created from it borrows its data") {
            const rerun::Collection<Position2D> batch(components);
            CHECK(batch.size() == components.size());
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from it moving it owns the data") {
            const rerun::Collection<Position2D> batch(std::move(components));
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a temporary vector of components") {
        THEN("a collection created from it owns its data") {
            const rerun::Collection<Position2D> batch(std::vector<Position2D>{
                Position2D(0.0f, 1.0f),
                Position2D(1.0f, 2.0f),
            });
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("an std::array of components") {
        std::array<Position2D, 2> components = {
            Position2D(0.0f, 1.0f),
            Position2D(1.0f, 2.0f),
        };

        THEN("a collection created from it borrows its data") {
            const rerun::Collection<Position2D> batch(components);
            CHECK(batch.size() == components.size());
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from it moving it owns the data") {
            const rerun::Collection<Position2D> batch(std::move(components));
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("a temporary std::array of components") {
        THEN("a collection created from it owns its data") {
            const rerun::Collection<Position2D> batch(std::array<Position2D, 2>{
                Position2D(0.0f, 1.0f),
                Position2D(1.0f, 2.0f),
            });
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a c-array of components") {
        Position2D components[] = {
            Position2D(0.0f, 1.0f),
            Position2D(1.0f, 2.0f),
        };

        THEN("a collection created from it borrows its data") {
            const rerun::Collection<Position2D> batch(components);
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from moving it owns the data") {
            const rerun::Collection<Position2D> batch(std::move(components));
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a single components") {
        Position2D component = Position2D(0.0f, 1.0f);

        THEN("a collection created from it borrows its data") {
            const rerun::Collection<Position2D> batch(component);
            CHECK(batch.size() == 1);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from it moving it owns the data") {
            const rerun::Collection<Position2D> batch(std::move(component));
            CHECK(batch.size() == 1);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("a single temporary component") {
        THEN("a collection created from it borrows its data") {
            const rerun::Collection<Position2D> batch(Position2D(0.0f, 1.0f));
            CHECK(batch.size() == 1);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
}

struct MyVec2Container {
    std::vector<float> vecs;
};

namespace rerun {
    template <>
    struct CollectionAdapter<components::Position2D, MyVec2Container> {
        // We're using the void* version of `borrow` which doesn't do these checks for us.
        static_assert(sizeof(components::Position2D) == sizeof(float) * 2);
        static_assert(alignof(components::Position2D) <= alignof(float));

        Collection<components::Position2D> operator()(const MyVec2Container& container) {
            return Collection<components::Position2D>::borrow(
                reinterpret_cast<const void*>(container.vecs.data()),
                container.vecs.size() / 2
            );
        }

        Collection<components::Position2D> operator()(MyVec2Container&&) {
            throw std::runtime_error("Not implemented for temporaries");
        }
    };
} // namespace rerun

SCENARIO("Collection creation via a custom adapter for a datalayout compatible type", TEST_TAG) {
    GIVEN("A custom vec2 container with a defined adapter") {
        MyVec2Container container;
        container.vecs = {0.0f, 1.0f, 2.0f, 3.0f};

        THEN("a collection created from it that its data") {
            const rerun::Collection<Position2D> batch(container);
            CHECK(batch.size() == 2);
            CHECK(batch.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("A Point2D archetype can be directly created from this container") {
            const rerun::archetypes::Points2D from_custom_container(container);

            CHECK(from_custom_container.positions.size() == 2);
            CHECK(
                from_custom_container.positions.get_ownership() ==
                rerun::CollectionOwnership::Borrowed
            );

            AND_THEN("it can be serialized and is identical to creation from rerun types directly"
            ) {
                const rerun::archetypes::Points2D from_rerun_vector({{0.0f, 1.0f}, {2.0f, 3.0f}});

                CHECK(from_rerun_vector.positions.size() == 2);
                CHECK(
                    from_rerun_vector.positions.get_ownership() ==
                    rerun::CollectionOwnership::VectorOwned
                );

                test_compare_archetype_serialization(from_custom_container, from_rerun_vector);
            }
        }
    }
}

SCENARIO("Collection move behavior", TEST_TAG) {
    std::vector<Position2D> components = {
        Position2D(0.0f, 1.0f),
        Position2D(1.0f, 2.0f),
    };

    GIVEN("A borrowed collection") {
        auto borrowed = rerun::Collection<Position2D>::borrow(components.data(), 2);

        THEN("then moving to a new batch moves the data and clears the source") {
            auto target(std::move(borrowed));
            CHECK(target.size() == 2);
            CHECK(target.get_ownership() == rerun::CollectionOwnership::Borrowed);
            CHECK(borrowed.size() == 0);
            CHECK(borrowed.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }

        THEN("moving it to an owned collection swaps their data") {
            auto target = rerun::Collection<Position2D>::take_ownership(std::vector(components));

            target = std::move(borrowed);
            CHECK(target.size() == 2);
            CHECK(target.get_ownership() == rerun::CollectionOwnership::Borrowed);
            CHECK(borrowed.size() == 2);
            CHECK(borrowed.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
        THEN("moving it to an borrowed collection swaps their data") {
            auto target = rerun::Collection<Position2D>::borrow(components.data(), 2);

            target = std::move(borrowed);
            CHECK(target.size() == 2);
            CHECK(target.get_ownership() == rerun::CollectionOwnership::Borrowed);
            CHECK(borrowed.size() == 2);
            CHECK(borrowed.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
    }
    GIVEN("A owned collection") {
        auto borrowed = rerun::Collection<Position2D>::take_ownership(std::vector(components));

        THEN("moving it to an owned collection swaps their data") {
            auto target = rerun::Collection<Position2D>::take_ownership(std::vector(components));

            target = std::move(borrowed);
            CHECK(target.size() == 2);
            CHECK(target.get_ownership() == rerun::CollectionOwnership::VectorOwned);
            CHECK(borrowed.size() == 2);
            CHECK(borrowed.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
        THEN("moving it to an borrowed collection swaps their data") {
            auto target = rerun::Collection<Position2D>::borrow(components.data(), 2);

            target = std::move(borrowed);
            CHECK(target.size() == 2);
            CHECK(target.get_ownership() == rerun::CollectionOwnership::VectorOwned);
            CHECK(borrowed.size() == 2);
            CHECK(borrowed.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
    }

    // Uncomment to check if the error message for missing adapter is sane:
    //std::vector<std::string> strings = {"a", "b", "c"};
    //rerun::Collection<Position2D> batch(strings);
}

// TODO: do one on copy behavior!
// TODO: copy_to_vector
