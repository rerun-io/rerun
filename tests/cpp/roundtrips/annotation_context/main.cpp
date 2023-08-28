#include <rerun/archetypes/annotation_context.hpp>
#include <rerun/recording_stream.hpp>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec_stream = rr::RecordingStream("rerun_example_roundtrip_annotation_context");
    rec_stream.save(argv[1]).throw_on_failure();

    rec_stream.log(
        "annotation_context",
        rr::archetypes::AnnotationContext({
            rr::datatypes::ClassDescription({1, "hello"}),
            rr::datatypes::ClassDescription(
                {2, "world", rr::datatypes::Color(3, 4, 5)},
                {{17, "head"}, {42, "shoulders"}},
                {
                    {1, 2},
                    {3, 4},
                }
            ),
        })
    );
}
