#include "archetype_test.hpp"

#include <rerun/archetypes/tensor.hpp>
using namespace rerun::archetypes;

#define TEST_TAG "[tensor][archetypes]"

SCENARIO("Tensor archetype can be created from tensor data." TEST_TAG) {
    GIVEN("a vector of data") {
        std::vector<int8_t> data(2 * 2 * 2 * 2, 0);
        THEN("no error occurs on image construction with either the vector or a data pointer") {
            auto image_from_vector = Tensor({2, 2, 2, 2}, data);
            auto image_from_ptr = Tensor({2, 2, 2, 2}, data.data());

            AND_THEN("serialization succeeds") {
                test_compare_archetype_serialization(image_from_ptr, image_from_vector);
            }
        }

        THEN(
            "adding tensor dimension names after the fact is the same as creating them from the start"
        ) {
            auto tensor_with_dim_names_with =
                Tensor({2, 2, 2, 2}, data).with_dim_names({"height", "width", "channel", "batch"});

            rerun::datatypes::TensorData tensor_with_dim_names_ctor_dt;
            tensor_with_dim_names_ctor_dt.buffer = data;
            tensor_with_dim_names_ctor_dt.shape = {2, 2, 2, 2};
            tensor_with_dim_names_ctor_dt.names = {"height", "width", "channel", "batch"};
            auto tensor_with_dim_names_ctor =
                rerun::archetypes::Tensor(tensor_with_dim_names_ctor_dt);

            test_compare_archetype_serialization(
                tensor_with_dim_names_ctor,
                tensor_with_dim_names_with
            );
        }
    }
}
