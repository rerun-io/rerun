import rerun as rr
import sys

rr.init("rerun_example_log_file", spawn=True)

rr.log_file_from_path(sys.argv[1])
