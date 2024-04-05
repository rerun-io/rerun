<!--[metadata]
title = "Multithreading"
description = "Demonstration of logging to Rerun from multiple threads."
thumbnail = "https://static.rerun.io/multithreading/80a3e566d6d9f8f17b04c839cd0ae2380c2baf02/480w.png"
thumbnail_dimensions = [480, 480]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/multithreading/8521bf95a7ff6004c932e8fb72429683928fbab4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/multithreading/8521bf95a7ff6004c932e8fb72429683928fbab4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/multithreading/8521bf95a7ff6004c932e8fb72429683928fbab4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/multithreading/8521bf95a7ff6004c932e8fb72429683928fbab4/1200w.png">
  <img src="https://static.rerun.io/multithreading/8521bf95a7ff6004c932e8fb72429683928fbab4/full.png" alt="Multithreading example screenshot">
</picture>

Demonstration of logging to Rerun from multiple threads.

# Used Rerun types
[`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d)

# Logging and visualizing with Rerun
This example showcases logging from multiple threads, starting with the definition of the function for logging, the `rect_logger`, followed by typical usage of Python's `threading` module in the main function.

 ```python
def rect_logger(path: str, color: npt.NDArray[np.float32]) -> None:
    for _ in range(1000):
        rects_xy = np.random.rand(5, 2) * 1024
        rects_wh = np.random.rand(5, 2) * (1024 - rects_xy + 1)
        rects = np.hstack((rects_xy, rects_wh))
        rr.log(path, rr.Boxes2D(array=rects, array_format=rr.Box2DFormat.XYWH, colors=color)) # Log the rectangles using Rerun
 ```

The main function manages the multiple threads for logging data to the Rerun viewer.
 ```python
def main() -> None:
    # … existing code …

    threads = []

    for i in range(10): # Create 10 threads to run the rect_logger function with different paths and colors.
        t = threading.Thread(target=rect_logger, args=(f"thread/{i}", [random.randrange(255) for _ in range(3)]))
        t.start()
        threads.append(t)

    for t in threads: # Wait for all threads to complete before proceeding.
        t.join()

    # … existing code …
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
pip install -r examples/python/multithreading/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/multithreading/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/multithreading/main.py --help
```
