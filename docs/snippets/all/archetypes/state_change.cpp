// Log a `StateChange`

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_state_change");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("step", 0);
    rec.log("door", rerun::StateChange().with_state("open"));

    rec.set_time_sequence("step", 1);
    rec.log("door", rerun::StateChange().with_state("closed"));

    rec.set_time_sequence("step", 2);
    rec.log("door", rerun::StateChange().with_state("open"));
}
