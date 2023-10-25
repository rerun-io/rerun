#include <rerun.h>
#include <stdio.h>

int main(void) {
    rr_error error = {0};
    rr_spawn(NULL, &error);

    if (error.code != 0) {
        printf("Error occurred: %s\n", error.description);
        return 1;
    }
}
