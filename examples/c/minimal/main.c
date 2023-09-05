#include <rerun.h>
#include <stdio.h>

int main(void) {
    printf("Rerun C SDK Version: %s\n", rr_version_string());

    rr_error error = {0};
    const rr_store_info store_info = {
        .application_id = "c-example-app",
        .store_kind = RERUN_STORE_KIND_RECORDING,
    };
    rr_recording_stream rec = rr_recording_stream_new(&store_info, &error);
    rr_recording_stream_connect(rec, "127.0.0.1:9876", 2.0, &error);

    if (error.code != 0) {
        printf("Error occurred: %s\n", error.description);
        return 1;
    }

    printf("rec: %d\n", rec);

    rr_recording_stream_free(rec);
}
