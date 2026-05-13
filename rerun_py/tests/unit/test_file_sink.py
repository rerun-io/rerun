"""Tests for the `FileSink` / `save` / `stdout` `write_footer` opt-out."""

from __future__ import annotations

from typing import TYPE_CHECKING

import rerun as rr

if TYPE_CHECKING:
    import pathlib

APP_ID = "rerun_example_test_file_sink"

# The trailing RRD `StreamFooter` frame always ends with the bytes `RRF2` followed by
# `FOOT`, located at `file_len - 12 .. file_len - 4`.
# See `re_log_encoding::rrd::frames::StreamFooter` for the definition.
_STREAM_FOOTER_FOURCC = b"RRF2"
_STREAM_FOOTER_IDENTIFIER = b"FOOT"


def _has_stream_footer(path: pathlib.Path) -> bool:
    """Return True if the file at `path` ends with a valid RRD `StreamFooter` trailer."""
    data = path.read_bytes()
    if len(data) < 12:
        return False
    return data[-12:-8] == _STREAM_FOOTER_FOURCC and data[-8:-4] == _STREAM_FOOTER_IDENTIFIER


def _log_some(rec: rr.RecordingStream) -> None:
    for i in range(10):
        rec.log("signal", rr.Scalars(float(i)))


def test_save_default_writes_footer(tmp_path: pathlib.Path) -> None:
    """`RecordingStream.save(path)` defaults to writing a footer."""
    rrd = tmp_path / "default.rrd"
    rec = rr.RecordingStream(APP_ID)
    rec.save(rrd)
    _log_some(rec)
    rec.disconnect()

    assert _has_stream_footer(rrd), "default save() must produce a footer-bearing file"


def test_save_write_footer_false_omits_footer(tmp_path: pathlib.Path) -> None:
    """`RecordingStream.save(path, write_footer=False)` produces a footer-less file."""
    rrd = tmp_path / "no_footer.rrd"
    rec = rr.RecordingStream(APP_ID)
    rec.save(rrd, write_footer=False)
    _log_some(rec)
    rec.disconnect()

    assert not _has_stream_footer(rrd), "save(…, write_footer=False) must produce a footer-less file"


def test_module_save_write_footer_false(tmp_path: pathlib.Path) -> None:
    """The module-level `rr.save(…, write_footer=False)` honours the flag."""
    rrd = tmp_path / "module_no_footer.rrd"
    rr.init(APP_ID + "_module")
    rr.save(rrd, write_footer=False)
    for i in range(10):
        rr.log("signal", rr.Scalars(float(i)))
    rr.disconnect()

    assert not _has_stream_footer(rrd)


def test_filesink_class_default_writes_footer(tmp_path: pathlib.Path) -> None:
    """The `rr.FileSink(path)` class defaults to writing a footer (legacy call shape)."""
    rrd = tmp_path / "filesink_default.rrd"
    rec = rr.RecordingStream(APP_ID)
    rec.set_sinks(rr.FileSink(rrd))
    _log_some(rec)
    rec.disconnect()

    assert _has_stream_footer(rrd)


def test_filesink_class_write_footer_false(tmp_path: pathlib.Path) -> None:
    """The `rr.FileSink(path, write_footer=False)` class honours the kw-only flag."""
    rrd = tmp_path / "filesink_no_footer.rrd"
    rec = rr.RecordingStream(APP_ID)
    rec.set_sinks(rr.FileSink(rrd, write_footer=False))
    _log_some(rec)
    rec.disconnect()

    assert not _has_stream_footer(rrd)
