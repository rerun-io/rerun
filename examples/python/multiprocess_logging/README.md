<!--[metadata]
title = "Multiprocess logginge"
thumbnail = "https://static.rerun.io/multiprocessing/959e2c675f52a7ca83e11e5170903e8f0f53f5ed/480w.png"
thumbnail_dimensions = [480, 480]
tags = ["API example"]
-->

Demonstrates how rerun can work with the python `multiprocessing` library.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/multiprocessing/72bcb7550d84f8e5ed5a39221093239e655f06de/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/multiprocessing/72bcb7550d84f8e5ed5a39221093239e655f06de/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/multiprocessing/72bcb7550d84f8e5ed5a39221093239e655f06de/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/multiprocessing/72bcb7550d84f8e5ed5a39221093239e655f06de/1200w.png">
  <img src="https://static.rerun.io/multiprocessing/72bcb7550d84f8e5ed5a39221093239e655f06de/full.png" alt="">
</picture>

# Used Rerun types
[`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log)

# Logging and visualizing with Rerun
This example demonstrates how to use the Rerun SDK with `multiprocessing` to log data from multiple processes to the same Rerun viewer.
It starts with the definition of the function for logging, the `task`, followed by typical usage of Python's `multiprocessing` library.

The function `task` is decorated with `@rr.shutdown_at_exit`. This decorator ensures that data is flushed when the task completes, even if the normal `atexit`-handlers are not called at the termination of a multiprocessing process.

```python
@rr.shutdown_at_exit
def task(child_index: int) -> None:
    rr.init("rerun_example_multiprocessing")

    rr.connect()

    title = f"task_{child_index}"
    rr.log(
        "log",
        rr.TextLog(
            f"Logging from pid={os.getpid()}, thread={threading.get_ident()} using the Rerun recording id {rr.get_recording_id()}"
        )
    )
    if child_index == 0:
        rr.log(title, rr.Boxes2D(array=[5, 5, 80, 80], array_format=rr.Box2DFormat.XYWH, labels=title))
    else:
        rr.log(
            title,
            rr.Boxes2D(
                array=[10 + child_index * 10, 20 + child_index * 5, 30, 40],
                array_format=rr.Box2DFormat.XYWH,
                labels=title,
            ),
        )
```

The main function initializes rerun with a specific application ID and manages the multiprocessing processes for logging data to the Rerun viewer.

> Caution: Ensure that the `recording id` specified in the main function matches the one used in the logging functions
 ```python
def main() -> None:
    # … existing code …

    rr.init("rerun_example_multiprocessing")
    rr.spawn(connect=False)  # this is the viewer that each child process will connect to

    task(0)

    for i in [1, 2, 3]:
        p = multiprocessing.Process(target=task, args=(i,))
        p.start()
        p.join()
 ```

# Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/multiprocessing/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/multiprocessing/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/multiprocessing/main.py --help
```
