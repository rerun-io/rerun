#include <catch2/catch_test_macros.hpp>

#include <rerun/archetypes/points2d.hpp>
#include <rerun/collection.hpp>
#include <rerun/collection_adapter_builtins.hpp>
#include <rerun/components/position2d.hpp>

#include "archetypes/archetype_test.hpp"

#define TEST_TAG "[collection]"

using namespace rerun::components;

// Input type that can be converted to the one held by the container.
struct ConvertibleElement {
    ConvertibleElement(int v) : value(v) {}

    ConvertibleElement(const ConvertibleElement& e) = default;

    ConvertibleElement& operator=(const ConvertibleElement& e) = default;

    int value = 99999;
};

// Type held by the container.
struct Element {
    Element(int v) : value(v) {}

    Element(const Element& e) : value(e.value) {
        ++copy_count;
    }

    Element(Element&& e) : value(e.value) {
        ++move_count;
    }

    Element& operator=(const Element& e) = default;

    Element(const ConvertibleElement& e) : value(e.value) {
        ++move_convertible_count;
    }

    Element(ConvertibleElement&& e) : value(e.value) {
        ++copy_convertible_count;
    }

    bool operator==(const Element& other) const {
        return value == other.value;
    }

    int value = 99999;

    static int move_count;
    static int copy_count;
    static int move_convertible_count;
    static int copy_convertible_count;
};

int Element::move_count = 0;
int Element::copy_count = 0;
int Element::move_convertible_count = 0;
int Element::copy_convertible_count = 0;

struct CheckElementMoveAndCopyCount {
    CheckElementMoveAndCopyCount()
        : copy_count_before(Element::copy_count),
          move_count_before(Element::move_count),
          copy_convertible_count_before(Element::copy_convertible_count),
          move_convertible_count_before(Element::move_convertible_count) {}

    ~CheckElementMoveAndCopyCount() {
// Both moves and copies can be elided, so we can only check for a minimum.
// But in debug builds this seems to be surprisingly reliable!
#ifdef NDEBUG
#define CMP >=
#else
#define CMP ==
#endif

        CHECK(Element::copy_count - copy_count_before CMP expected_copy_increase);
        CHECK(Element::move_count - move_count_before CMP expected_move_increase);
        CHECK(
            Element::copy_convertible_count -
            copy_convertible_count_before CMP expected_copy_convertible_increase
        );
        CHECK(
            Element::move_convertible_count -
            move_convertible_count_before CMP expected_move_convertible_increase
        );
#undef CMP
    }

    CheckElementMoveAndCopyCount(const CheckElementMoveAndCopyCount&) = delete;

    CheckElementMoveAndCopyCount& expect_move(int i) {
        expected_move_increase = i;
        return *this;
    }

    CheckElementMoveAndCopyCount& expect_copy(int i) {
        expected_copy_increase = i;
        return *this;
    }

    CheckElementMoveAndCopyCount& expect_convertible_move(int i) {
        expected_copy_convertible_increase = i;
        return *this;
    }

    CheckElementMoveAndCopyCount& expect_convertible_copy(int i) {
        expected_move_convertible_increase = i;
        return *this;
    }

    int expected_copy_increase = 0;
    int expected_move_increase = 0;
    int expected_copy_convertible_increase = 0;
    int expected_move_convertible_increase = 0;

    int copy_count_before, move_count_before;
    int copy_convertible_count_before, move_convertible_count_before;
};

#define EXPECTED_ELEMENT_LIST {1337, 42}

// Checks if the collection contains the elements defined in `EXPECTED_ELEMENT_LIST`.
void check_for_expected_list(const rerun::Collection<Element>& collection) {
    std::array<Element, 2> expected = EXPECTED_ELEMENT_LIST;
    CHECK(collection.size() == expected.size());
    CHECK(collection[0] == expected[0]);
    CHECK(collection[1] == expected[1]);
}

#define EXPECTED_SINGLE 666

// Checks if the collection contains the elements defined in `EXPECTED_SINGLE`.
void check_for_expected_single(const rerun::Collection<Element>& collection) {
    Element expected = EXPECTED_SINGLE;
    CHECK(collection.size() == 1);
    CHECK(collection[0] == expected);
}

SCENARIO("Default constructing a collection", TEST_TAG) {
    GIVEN("a default constructed collection") {
        rerun::Collection<Element> collection;

        THEN("it is empty") {
            CHECK(collection.size() == 0);
            CHECK(collection.empty());
        }
        THEN("it is borrowed") {
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
    }
}

SCENARIO(
    "Collection creation via basic adapters, using the container's value_type as input", TEST_TAG
) {
    GIVEN("a vector of elements") {
        std::vector<Element> elements = EXPECTED_ELEMENT_LIST;

        THEN("a collection created from it borrows its data") {
            CheckElementMoveAndCopyCount check; // No copies or moves.

            const rerun::Collection<Element> collection(elements);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from moving it owns the data") {
            CheckElementMoveAndCopyCount check;
            // No element copies or moves, the vector itself is moved.

            const rerun::Collection<Element> collection(std::move(elements));
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a temporary vector of elements") {
        THEN("a collection created from it owns its data") {
            CheckElementMoveAndCopyCount check;
            // No element moves, the vector itself is moved.
            check.expect_copy(2); // for constructing the temporary vector.

            const rerun::Collection<Element> collection(std::vector<Element> EXPECTED_ELEMENT_LIST);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("an std::array of elements") {
        std::array<Element, 2> elements = EXPECTED_ELEMENT_LIST;

        THEN("a collection created from it borrows its data") {
            CheckElementMoveAndCopyCount check; // No copies or moves.

            const rerun::Collection<Element> collection(elements);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from it moving it owns the data") {
            CheckElementMoveAndCopyCount check;
            check.expect_move(2);

            const rerun::Collection<Element> collection(std::move(elements));
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("a temporary std::array of elements") {
        THEN("a collection created from it owns its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_move(2);

            const rerun::Collection<Element> collection(std::array<Element, 2> EXPECTED_ELEMENT_LIST
            );
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a c-array of elements") {
        Element elements[] = EXPECTED_ELEMENT_LIST;

        THEN("a collection created from it borrows its data") {
            CheckElementMoveAndCopyCount check; // No copies or moves.

            const rerun::Collection<Element> collection(elements);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from moving it owns the data") {
            CheckElementMoveAndCopyCount check;
            check.expect_move(2);

            const rerun::Collection<Element> collection(std::move(elements));
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a single element") {
        Element component = EXPECTED_SINGLE;

        THEN("a collection created from it borrows its data") {
            CheckElementMoveAndCopyCount check; // No copies or moves.

            const rerun::Collection<Element> collection(component);
            check_for_expected_single(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN("a collection created from moving it owns the data") {
            CheckElementMoveAndCopyCount check;
            check.expect_move(1);

            const rerun::Collection<Element> collection(std::move(component));
            check_for_expected_single(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("a single temporary component") {
        THEN("a collection created from it owns the data") {
            CheckElementMoveAndCopyCount check;
            check.expect_move(1);

            const rerun::Collection<Element> collection(Element(EXPECTED_SINGLE));
            check_for_expected_single(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
}

SCENARIO(
    "Collection creation via basic adapters, using a type that is compatible to the container's value_type as input",
    TEST_TAG
) {
    GIVEN("a vector of convertible elements") {
        std::vector<ConvertibleElement> elements = EXPECTED_ELEMENT_LIST;

        THEN("a collection created from it copies its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_copy(2);

            const rerun::Collection<Element> collection(elements);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
        THEN("a collection created from it moves its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_move(2);

            const rerun::Collection<Element> collection(std::move(elements));
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("a temporary vector of convertible elements") {
        THEN("a collection created from it moves its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_move(2);

            const rerun::Collection<Element>
                collection(std::vector<ConvertibleElement> EXPECTED_ELEMENT_LIST);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    GIVEN("an std::array of convertible elements") {
        std::array<ConvertibleElement, 2> elements = EXPECTED_ELEMENT_LIST;

        THEN("a collection created from it copies its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_copy(2);

            const rerun::Collection<Element> collection(elements);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
        THEN("a collection created from it moves its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_move(2);

            const rerun::Collection<Element> collection(std::move(elements));
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("a temporary std::array of convertible elements") {
        THEN("a collection created from it moves its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_move(2);

            const rerun::Collection<Element>
                collection(std::array<ConvertibleElement, 2> EXPECTED_ELEMENT_LIST);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }

    // Not yet supported.
    // GIVEN("a c-array of convertible elements") {
    //     ConvertibleElement elements[] = EXPECTED_ELEMENT_LIST;
    //
    //     THEN("a collection created from it borrows its data") {
    //     }
    //     THEN("a collection created from moving it owns the data") {
    //     }
    // }

    GIVEN("a single convertible element") {
        ConvertibleElement element = EXPECTED_SINGLE;

        THEN("a collection created from it copies its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_copy(1);
            // The resulting value is moved internally into a vector.
            check.expect_move(1);

            const rerun::Collection<Element> collection(element);
            check_for_expected_single(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
        THEN("a collection created from it move its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_move(1);
            // The resulting value is moved internally into a vector.
            check.expect_move(1);

            const rerun::Collection<Element> collection(std::move(element));
            check_for_expected_single(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("a single temporary convertible element") {
        THEN("a collection created from it move its data") {
            CheckElementMoveAndCopyCount check;
            check.expect_convertible_move(1);
            // The resulting value is moved internally into a vector.
            check.expect_move(1);

            const rerun::Collection<Element> collection(ConvertibleElement(EXPECTED_SINGLE));
            check_for_expected_single(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
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

            CHECK(from_custom_container.positions.has_value());

            AND_THEN("it can be serialized and is identical to creation from rerun types directly"
            ) {
                const rerun::archetypes::Points2D from_rerun_vector({{0.0f, 1.0f}, {2.0f, 3.0f}});

                test_compare_archetype_serialization(from_custom_container, from_rerun_vector);
            }
        }
    }
}

SCENARIO("Move construction/assignment of collections", TEST_TAG) {
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
            CHECK(borrowed.empty());

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

SCENARIO("Copy/move construction/assignment of collections", TEST_TAG) {
    GIVEN("A default constructed collection") {
        rerun::Collection<int> collection;
        const int* old_data_ptr = collection.data();

        THEN("it can be move constructed") {
            rerun::Collection<int> collection2(std::move(collection));
            CHECK(collection2.size() == 0);
            CHECK(collection2.empty());

            CHECK(collection2.data() == old_data_ptr);
        }
        THEN("it can be move assigned") {
            rerun::Collection<int> collection2;
            collection2 = std::move(collection);
            CHECK(collection2.size() == 0);
            CHECK(collection2.empty());

            CHECK(collection2.data() == old_data_ptr);
        }

        THEN("it can be copy constructed") {
            rerun::Collection<int> collection2(collection);
            CHECK(collection2.size() == 0);
            CHECK(collection2.empty());
        }
        THEN("it can be copy assigned") {
            rerun::Collection<int> collection2;
            collection2 = collection;
            CHECK(collection2.size() == 0);
            CHECK(collection2.empty());
        }
    }

    GIVEN("a collection with owned data") {
        auto collection =
            rerun::Collection<Element>::take_ownership(std::vector<Element> EXPECTED_ELEMENT_LIST);
        const Element* old_data_ptr = collection.data();

        THEN("it can be move constructed") {
            CheckElementMoveAndCopyCount check; // No move or copy.

            rerun::Collection<Element> collection2(std::move(collection));
            check_for_expected_list(collection2);
            CHECK(collection2.data() == old_data_ptr);
        }
        THEN("it can be move assigned") {
            CheckElementMoveAndCopyCount check; // No move or copy.

            rerun::Collection<Element> collection2;
            collection2 = std::move(collection);
            check_for_expected_list(collection2);
            CHECK(collection2.data() == old_data_ptr);
        }

        THEN("it can be copy constructed") {
            CheckElementMoveAndCopyCount check;
            check.expect_copy(2);

            rerun::Collection<Element> collection2(collection);
            check_for_expected_list(collection2);
        }
        THEN("it can be copy assigned") {
            CheckElementMoveAndCopyCount check;
            check.expect_copy(2);

            rerun::Collection<Element> collection2;

            collection2 = collection;
            check_for_expected_list(collection2);
        }
    }

    GIVEN("a collection with borrowed data") {
        std::vector<Element> data EXPECTED_ELEMENT_LIST;
        auto collection = rerun::Collection<Element>::borrow(data.data(), data.size());
        const Element* old_data_ptr = data.data();

        THEN("it can be move constructed") {
            CheckElementMoveAndCopyCount check; // No move or copy.

            rerun::Collection<Element> collection2(std::move(collection));
            check_for_expected_list(collection2);
            CHECK(collection2.data() == old_data_ptr);
        }
        THEN("it can be move assigned") {
            CheckElementMoveAndCopyCount check; // No move or copy.

            rerun::Collection<Element> collection2;
            collection2 = std::move(collection);
            check_for_expected_list(collection2);
            CHECK(collection2.data() == old_data_ptr);
        }

        THEN("it can be copy constructed") {
            CheckElementMoveAndCopyCount check; // No move or copy.

            rerun::Collection<Element> collection2(collection);
            check_for_expected_list(collection2);
        }
        THEN("it can be copy assigned") {
            CheckElementMoveAndCopyCount check; // No move or copy.

            rerun::Collection<Element> collection2;

            collection2 = collection;
            check_for_expected_list(collection2);
        }
    }
}

SCENARIO("Conversion to vector using `to_vector`", TEST_TAG) {
    auto expected_vector = std::vector<Element> EXPECTED_ELEMENT_LIST;

    GIVEN("a collection with owned data") {
        auto collection =
            rerun::Collection<Element>::take_ownership(std::vector<Element> EXPECTED_ELEMENT_LIST);

        THEN("it can be converted to a vector") {
            CheckElementMoveAndCopyCount check;
            check.expect_copy(2);

            CHECK(collection.to_vector() == expected_vector);
        }

        THEN("it can be moved to a vector, resulting in no copies") {
            CheckElementMoveAndCopyCount check;

            CHECK(std::move(collection).to_vector() == expected_vector);
        }
    }
    GIVEN("a collection with borrowed data") {
        std::vector<Element> data EXPECTED_ELEMENT_LIST;
        auto collection = rerun::Collection<Element>::borrow(data.data(), data.size());

        THEN("it can be converted to a vector") {
            CheckElementMoveAndCopyCount check;
            check.expect_copy(2);

            CHECK(collection.to_vector() == expected_vector);
        }

        THEN("it can be moved to a vector, resulting in copies") {
            CheckElementMoveAndCopyCount check;
            check.expect_copy(2);

            CHECK(std::move(collection).to_vector() == expected_vector);
        }
    }
}

SCENARIO("Borrow and take ownership if easy with the free utility functions") {
    GIVEN("A vector") {
        std::vector<Element> data EXPECTED_ELEMENT_LIST;

        THEN("it can be borrowed without via `rerun::borrow` without specifying template arguments"
        ) {
            CheckElementMoveAndCopyCount check; // No element copies or moves expected.

            const auto collection = rerun::borrow(data);
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
        THEN(
            "it can be taken ownership via `rerun::take_ownership` without specifying template arguments"
        ) {
            CheckElementMoveAndCopyCount check; // No element copies or moves expected.

            const auto collection = rerun::take_ownership(std::move(data));
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
        }
    }
    GIVEN("A pointer to an array") {
        std::array<Element, 2> data EXPECTED_ELEMENT_LIST;

        THEN("it can be borrowed via `rerun::borrow` without specifying template arguments") {
            CheckElementMoveAndCopyCount check; // No element copies or moves expected.

            const auto collection = rerun::borrow(data.data(), data.size());
            check_for_expected_list(collection);
            CHECK(collection.get_ownership() == rerun::CollectionOwnership::Borrowed);
        }
    }
    GIVEN("A single element") {
        Element data = EXPECTED_SINGLE;

        THEN(
            "it can be taken ownership via `rerun::take_ownership` without specifying template arguments"
        ) {
            WHEN("passed by value") {
                CheckElementMoveAndCopyCount check;
                check.expect_copy(1); // copy on call
                check.expect_move(1); // move to rerun::Collection

                const auto collection = rerun::take_ownership(data);
                check_for_expected_single(collection);
                CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
            }
            WHEN("passed moved") {
                CheckElementMoveAndCopyCount check;
                check.expect_move(2); // move on call, move to rerun::Collection

                const auto collection = rerun::take_ownership(std::move(data));
                check_for_expected_single(collection);
                CHECK(collection.get_ownership() == rerun::CollectionOwnership::VectorOwned);
            }
        }
    }
}
