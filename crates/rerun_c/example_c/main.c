#include <rerun.h>
#include <stdio.h>

int main(void) {
    printf("Rerun C SDK Version: %s\n", rerun_version_string());

    const struct RerunStoreInfo store_info = {
        .application_id = "c-example-app",
        .store_kind = RERUN_STORE_KIND_RECORDING,
    };
    RerunRecStream rec_stream =
        rerun_rec_stream_new(&store_info, "0.0.0.0:9876");

    printf("rec_stream: %d\n", rec_stream);

    rerun_rec_stream_free(rec_stream);
}
