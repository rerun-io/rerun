// Log a `Status`

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_status");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("step", 0);
    rec.log("door", rerun::Status().with_status("open"));

    rec.set_time_sequence("step", 1);
    rec.log("door", rerun::Status().with_status("closed"));

    rec.set_time_sequence("step", 2);
    rec.log("door", rerun::Status().with_status("open"));
}
