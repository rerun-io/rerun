#include <rerun.h>
#include <stdio.h>

int main(void) {
    printf("Rerun C SDK Version: %s\n", rr_version_string());

    const rr_store_info store_info = {
        .application_id = "c-example-app",
        .store_kind = RERUN_STORE_KIND_RECORDING,
    };
    rr_recording_stream rec_stream = rr_recording_stream_new(&store_info, NULL);
    rr_recording_stream_connect(rec_stream, "127.0.0.1:9876", 2.0);

    printf("rec_stream: %d\n", rec_stream);

    rr_recording_stream_free(rec_stream);
}
