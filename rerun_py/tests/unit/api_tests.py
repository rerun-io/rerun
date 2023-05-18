import depthai_viewer as viewer
from depthai_viewer.log.text import LogLevel


def test_text() -> None:
    viewer.log_text_entry("path", "text", level=None)
    viewer.log_text_entry("path", "text", level=LogLevel.INFO)
    viewer.log_text_entry("path", None, level=LogLevel.INFO)  # type: ignore[arg-type]
