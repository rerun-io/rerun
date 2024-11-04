//! Log a simple undirected graph.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_graph_undirected");
    rec.spawn().exit_on_failure();

    rec.log(
        "simple",
        rerun::GraphNodes({{"a"}, {"b"}, {"c"}})
            .with_positions({{0.0, 100.0}, {-100.0, 0.0}, {100.0, 0.0}})
            .with_labels({"A", "B", "C"})
    );

    // TODO(grtlr): This still throws warnings!
    rerun::datatypes::Utf8Pair edge_1{.first{"a"}, .second{"b"}};
    rerun::datatypes::Utf8Pair edge_2{.first{"b"}, .second{"c"}};
    rerun::datatypes::Utf8Pair edge_3{.first{"c"}, .second{"a"}};

    // Note: We log to the same entity here.
    rec.log(
        "simple",
        rerun::GraphEdges({edge_1, edge_2, edge_3})
            // Optional: graphs are undirected by default.
            .with_graph_type(rerun::components::GraphType::Undirected)
    );
}
