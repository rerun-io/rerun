#include <rerun/archetypes/annotation_context.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_annotation_context");
    rec.save(argv[1]).exit_on_failure();

    rec.log(
        "annotation_context",
        rerun::archetypes::AnnotationContext({
            rerun::datatypes::ClassDescription(1, "hello"),
            rerun::datatypes::ClassDescription(
                {2, "world", rerun::datatypes::Rgba32(3, 4, 5)},
                {{17, "head"}, {42, "shoulders"}},
                {
                    {1, 2},
                    {3, 4},
                }
            ),
        })
    );
}
