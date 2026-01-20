//! Query and display the first 10 rows of a recording in a dataframe view.
//!
//! The blueprint is being loaded from an existing blueprint recording file.

// ./dataframe_view_query_external /tmp/dna.rrd /tmp/dna.rbl

#include <string>

#include <rerun.hpp>

int main(int argc, char** argv) {
    if (argc < 3) {
        return 1;
    }

    std::string path_to_rrd = argv[1];
    std::string path_to_rbl = argv[2];

    const auto rec = rerun::RecordingStream("rerun_example_dataframe_view_query_external");
    rec.spawn().exit_on_failure();

    // Log the files
    rec.log_file_from_path(path_to_rrd);
    rec.log_file_from_path(path_to_rbl);

    return 0;
}
