<!--[metadata]
title = "Shared recording"
-->


<picture>
  <img src="https://static.rerun.io/shared_recording/c3da85f1d4c158b8c7afb6bd3278db000b58049d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/shared_recording/c3da85f1d4c158b8c7afb6bd3278db000b58049d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/shared_recording/c3da85f1d4c158b8c7afb6bd3278db000b58049d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/shared_recording/c3da85f1d4c158b8c7afb6bd3278db000b58049d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/shared_recording/c3da85f1d4c158b8c7afb6bd3278db000b58049d/1200w.png">
</picture>

This example demonstrates how to use `RecordingId`s to create a single shared recording across multiple processes.

Run the following multiple times, and you'll see that each invocation adds data to the existing recording rather than creating a new one:
```bash
python examples/python/shared_recording/main.py
```
