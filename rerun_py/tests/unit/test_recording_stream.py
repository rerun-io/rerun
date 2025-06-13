from __future__ import annotations
import tempfile

import rerun as rr
import pyarrow as pa


def test_recording_stream_context_manager_removes_underlying_recording() -> None:
    app_id = "test_recording_stream_removal"
    rec_id = "same_id_all_the_time"

    # Time settings go to the underlying recording.
    # A new recording with the same id should not know about the time setting if it was removed.
    with rr.RecordingStream(app_id, recording_id=rec_id) as rec:
        rec.set_time("test_timeline", duration=1.0)

    with tempfile.TemporaryDirectory() as tmpdir:
        rrd = tmpdir + "/tmp.rrd"

        with rr.RecordingStream(app_id, recording_id=rec_id) as rec:
            rec.save(rrd)
            rec.log("log", rr.TextLog("Hello"))

        recording = rr.dataframe.load_recording(rrd)
        view = recording.view(index="log_time", contents="/**")
        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # Make sure `test_timeline` doesn't show up.
        print(table)
        assert table.num_columns == 3
        assert "log_tick" in table.column_names
        assert "log_time" in table.column_names
        assert "/log:TextLog:text" in table.column_names
