#include <iostream>
#include <rerun.hpp>

int main() {
    std::cout << "Testing Rerun SDK version: " << rerun::version_string() << std::endl;
    return 0;
}
