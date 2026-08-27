---
title: "Experimental dataloaders use a unified fetch and decode pipeline"
hidden: true
type: feature
---

### Experimental dataloaders use a unified fetch and decode pipeline

The experimental PyTorch dataloaders now use the same explicit stages to plan queries, fetch Arrow data, resolve decoder requests, and batch-decode samples.
This makes behavior consistent across iterable and map-style datasets as well as live-catalog and manifest-backed loading.

OpenTelemetry tracing for `RerunIterableDataset` now covers both fetch paths and distinguishes block fetching, batch decoding, exposed fetch latency, and downstream pull delays.
Decode spans end before samples are yielded, so training-loop behavior no longer inflates decode durations.

Docs: ../howto/train/dataloader.md
