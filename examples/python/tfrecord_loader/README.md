<!--[metadata]
title = "TFRecord loader"
source = "https://github.com/rerun-io/rerun-loader-python-example-tfrecord"
tags = ["2D", "Tensor", "Loader", "Time series"]
thumbnail = "https://static.rerun.io/tfrecord_loader/26da14f065a3d12322890d2842c988031113bd7b/480w.png"
thumbnail_dimensions = [480, 480]
-->

<picture>
  <img src="https://static.rerun.io/tfrecord_loader/98e2cbc73e61682f3932ed591f3b34bd512c1064/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/tfrecord_loader/98e2cbc73e61682f3932ed591f3b34bd512c1064/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/tfrecord_loader/98e2cbc73e61682f3932ed591f3b34bd512c1064/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/tfrecord_loader/98e2cbc73e61682f3932ed591f3b34bd512c1064/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/tfrecord_loader/98e2cbc73e61682f3932ed591f3b34bd512c1064/1200w.png">
</picture>


## Overview

This is an example data-loader plugin that lets you view a TFRecord of Events (i.e., Tensorboard log files). It uses the [external data loader mechanism](https://www.rerun.io/docs/reference/data-loaders/overview#external-dataloaders) to add this capability to the Rerun Viewer without modifying the Viewer itself.

This example is written in Python, and uses [TensorFlow](https://www.tensorflow.org/) to read the files. The events are then logged to Rerun.

**Note**: Not all events are supported yet. Scalars, images, text, and tensors should work. Unsupported events are skipped.


## Installing the plug-in

The [repository](https://github.com/rerun-io/rerun-loader-python-example-tfrecord) has detailed installation instruction. In a nutshell, the easiest is to use `pipx`:

```
pipx install git+https://github.com/rerun-io/rerun-loader-python-example-tfrecord.git
pipx ensurepath
```


## Try it out

To try the plug-in, first download an example `xxx.tfevents.xxx` file:

```bash
curl -OL https://github.com/rerun-io/rerun-loader-python-example-tfrecord/raw/main/events.tfevents.example
```

Then you can open the Viewer and open the file using drag-and-drop or the open dialog, or you can open it directly from the terminal:

```bash
rerun events.tfevents.example
```
