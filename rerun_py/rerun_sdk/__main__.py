"""
See `python3 -m rerun_sdk --help`
"""
import sys

from rerun_sdk import rerun_sdk as rerun_rs # type: ignore[attr-defined]

if __name__ == "__main__":
    rerun_rs.main(sys.argv)
