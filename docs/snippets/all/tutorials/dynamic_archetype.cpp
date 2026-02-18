// Log arbitrary archetype data.

#include <rerun.hpp>

#include <arrow/array/builder_binary.h>
#include <arrow/array/builder_primitive.h>
#include <cstdio>

arrow::Status run_main() {
    const auto rec = rerun::RecordingStream("rerun_example_dynamic_archetype");
    rec.spawn().exit_on_failure();

    std::shared_ptr<arrow::Array> arrow_array;

    arrow::DoubleBuilder confidences_builder;
    ARROW_RETURN_NOT_OK(confidences_builder.AppendValues({1.2, 3.4, 5.6}));
    ARROW_RETURN_NOT_OK(confidences_builder.Finish(&arrow_array));
    auto confidences = rerun::ComponentBatch::from_arrow_array(
        std::move(arrow_array),
        rerun::ComponentDescriptor("MyArchetype:confidence")
            .with_component_type(rerun::Loggable<rerun::components::Scalar>::ComponentType)
            .with_archetype("MyArchetype")
    );

    arrow::StringBuilder description_builder;
    ARROW_RETURN_NOT_OK(description_builder.Append("Bla bla bla…"));
    ARROW_RETURN_NOT_OK(description_builder.Finish(&arrow_array));
    auto description = rerun::ComponentBatch::from_arrow_array(
        std::move(arrow_array),
        rerun::ComponentDescriptor("MyArchetype:description")
            .with_component_type(

                rerun::Loggable<rerun::components::Text>::ComponentType
            )
            .with_archetype("MyArchetype")
    );
    // URIs will become clickable links
    arrow::StringBuilder homepage_builder;
    ARROW_RETURN_NOT_OK(homepage_builder.Append("https://www.rerun.io"));
    ARROW_RETURN_NOT_OK(homepage_builder.Finish(&arrow_array));
    auto homepage = rerun::ComponentBatch::from_arrow_array(
        std::move(arrow_array),
        rerun::ComponentDescriptor("MyArchetype:homepage").with_archetype("MyArchetype")
    );

    arrow::StringBuilder repository_builder;
    ARROW_RETURN_NOT_OK(repository_builder.Append("https://github.com/rerun-io/rerun"));
    ARROW_RETURN_NOT_OK(repository_builder.Finish(&arrow_array));
    auto repository = rerun::ComponentBatch::from_arrow_array(
        std::move(arrow_array),
        rerun::ComponentDescriptor("MyArchetype:repository").with_archetype("MyArchetype")
    );

    rec.log("new_archetype", confidences, description, homepage, repository);

    return arrow::Status::OK();
}

int main(int argc, char* argv[]) {
    arrow::Status status = run_main();
    if (!status.ok()) {
        printf("%s\n", status.ToString().c_str());
        return 1;
    }
    return 0;
}
