import rerun as rr
from rerun.log.text import LogLevel


def test_text() -> None:
    rr.log_text_entry("path", "text", level=None)
    rr.log_text_entry("path", "text", level=LogLevel.INFO)
    rr.log_text_entry("path", None, level=LogLevel.INFO)  # type: ignore[arg-type]
