#include <stdio.h>

#include <rerun.h>

int main() {
    printf("Hello from C!\n");
    printf("Rerun version: %s\n", rerun_version_string());

    const struct RerunStoreInfo store_info = {
        .application_id = "c-example-app",
        .store_kind = RERUN_STORE_KIND_RECORDING,
    };
    RerunRecStream rec_stream = rerun_rec_stream_new(&store_info, "0.0.0.0:9876");

    printf("rec_stream: %d\n", rec_stream);

    rerun_rec_stream_free(rec_stream);
}
