"""Log to one recording from several Python processes."""

import multiprocessing

import rerun as rr


# Forked processes skip the `atexit` handler that flushes data.
@rr.shutdown_at_exit
def task(index: int) -> None:
    # Same application ID as the parent.
    rr.init("rerun_example_multiprocessing_logging")
    rr.connect_grpc()
    rr.log(f"task_{index}", rr.TextLog(f"hello from task {index}"))


if __name__ == "__main__":
    rr.init("rerun_example_multiprocessing_logging")
    # Start a Viewer that the child processes connect to.
    rr.spawn(connect=False)

    for i in range(3):
        p = multiprocessing.Process(target=task, args=(i,))
        p.start()
        p.join()
