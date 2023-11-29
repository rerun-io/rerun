#include <unistd.h>
#include <iostream>
#include <sstream>

#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

using rerun::demo::grid3d;

int main() {
    const auto rec =
        rerun::RecordingStream("rerun_example_shared_recording", "my_shared_recording");
    rec.spawn().exit_on_failure();

    int pid = getpid();
    std::ostringstream oss;
    oss << "Hello from " << pid;

    rec.log("updates", rerun::TextLog(oss.str()));

    std::cout << "Run me again to append more data to the recording!" << std::endl;
}
