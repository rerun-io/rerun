from typing import Any, Dict

from depthai_viewer import bindings
from depthai_viewer.components.xlink_stats import XLinkStats
from depthai_viewer.log.log_decorator import log_decorator


@log_decorator
def log_xlink_stats(total_bytes_written: int, total_bytes_read: int, timestamp: float) -> None:
    """
    Log an XLink throughput statistic.

    Parameters
    ----------
    total_bytes_written:
        Total bytes written to the XLink by the host.
    total_bytes_read:
        Total bytes read from the XLink by the host.
    timestamp:
        Timestamp of the XLink throughput statistic in s since epoch.
    """
    instanced: Dict[str, Any] = {}
    instanced["rerun.xlink_stats"] = XLinkStats.create(
        total_bytes_written, total_bytes_read, timestamp
    )  # type: ignore[arg-type]
    bindings.log_arrow_msg("xlink_stats", components=instanced, timeless=False)
