#include <catch2/catch_session.hpp>
#include <loguru.hpp>

int main(int argc, char* argv[]) {
    loguru::g_preamble_uptime = false;
    loguru::g_preamble_thread = false;
    loguru::init(argc, argv); // installs signal handlers

    int result = Catch::Session().run(argc, argv);
    return result;
}
