#include <rerun.h>
#include <stdio.h>

int main(void) {
    printf("Rerun C SDK Version: %s\n", rr_version_string());

    const struct rr_store_info store_info = {
        .application_id = "c-example-app",
        .store_kind = RERUN_STORE_KIND_RECORDING,
    };
    rr_recording_stream rec_stream =
        rr_recording_stream_new(&store_info, "0.0.0.0:9876");

    printf("rec_stream: %d\n", rec_stream);

    rr_recording_stream_free(rec_stream);
}
