from __future__ import annotations

import gc

import rerun as rr

# If torch is available, use torch.multiprocessing instead of multiprocessing
# since it causes more issues. But, it's annoying to always require it so at
# least for the tests in other contexts, we'll use the standard library version.
try:
    from torch import multiprocessing
except ImportError:
    import multiprocessing


def task() -> None:
    gc.collect()


def test_multiprocessing_gc() -> None:
    rr.init("rerun_example_multiprocessing_gc")

    proc = multiprocessing.Process(
        target=task,
    )
    proc.start()
    proc.join(1)
    if proc.is_alive():
        # Terminate so our test doesn't get stuck
        proc.terminate()
        assert False, "Process deadlocked during gc.collect()"
