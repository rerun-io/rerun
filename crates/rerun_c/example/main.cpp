#include <stdio.h>

#include <rerun.h>

int main() {
    printf("Hello from C!\n");
    printf("Rerun version: %s\n", rerun_version_string());
    rerun_print_hello_world();
}
