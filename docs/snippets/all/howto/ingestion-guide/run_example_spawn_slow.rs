// Don't ever do this!
RERUN_FLUSH_NUM_ROWS=0 cargo r --release | RERUN_CHUNK_MAX_ROWS=0 rerun -
