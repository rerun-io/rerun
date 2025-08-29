#include <cstdio>
#include <rerun.hpp>

int main(int argc, char* argv[]) {
    if (argc != 2) {
        printf("Usage: %s <path_to_mcap>", argv[0]);
        return 1;
    }

    const std::string path_to_mcap = argv[1];

    // Initialize the SDK and give our recording a unique name
    const auto rec = rerun::RecordingStream("rerun_example_load_mcap");
    rec.spawn().exit_on_failure();

    // Load the MCAP file
    rec.log_file_from_path(path_to_mcap);

    return 0;
}
