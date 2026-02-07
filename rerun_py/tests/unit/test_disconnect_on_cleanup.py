from __future__ import annotations

import tempfile

import rerun as rr

import rerun_bindings  # noqa: TID251


def test_disconnect_on_cleanup() -> None:
    with tempfile.TemporaryDirectory() as dirpath:
        rec_path = f"{dirpath}/rec.rrd"

        def create_recording() -> None:
            rec = rr.RecordingStream("rerun_example_construct")
            rec.save(rec_path)
            rec.log("x", rr.Points2D(positions=[(1, 2), (3, 4)]))

        create_recording()

        assert rerun_bindings.check_for_rrd_footer(rec_path)


def test_disconnect_on_cleanup_with_ctx() -> None:
    with tempfile.TemporaryDirectory() as dirpath:
        rec_path = f"{dirpath}/rec.rrd"

        def create_recording() -> None:
            with rr.RecordingStream("rerun_example_ctx") as rec:
                rec.save(rec_path)
                rec.log("x", rr.Points2D(positions=[(1, 2), (3, 4)]))

        create_recording()

        assert rerun_bindings.check_for_rrd_footer(rec_path)


if __name__ == "__main__":
    test_disconnect_on_cleanup()
