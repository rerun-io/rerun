#include <iostream>

#define RERUN_WITH_ARROW 1

#include <rerun.hpp>

int main() {
  std::cerr << "Rerun C++ SDK version:" << rerun::version_string() << std::endl;

  auto buffer = rerun::create_buffer().ValueOrDie();

  std::cerr << "Buffer size: " << buffer->size() << " bytes." << std::endl;
}
