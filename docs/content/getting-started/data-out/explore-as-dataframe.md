---
title: Explore a recording with the dataframe view
order: 1
---

In this first part of the guide, we run the [face tracking example](https://rerun.io/examples/video-image/face_tracking) and explore the data in the viewer.

## Create a recording

The first step is to create a recording in the viewer using the face tracking example. Check the [face tracking installation instruction](https://rerun.io/examples/video-image/face_tracking#run-the-code) for more information on how to run this example.

Here is such a recording:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/getting-started-data-out/data-out-first-look-vp8.webm" type="video/webm" />
</video>

A person's face is visible and being tracked. Their jaws occasionally open and close. In the middle of the recording, the face is also temporarily hidden and no longer tracked.

## Explore the data

Amongst other things, the [MediaPipe Face Landmark](https://ai.google.dev/edge/mediapipe/solutions/vision/face_landmarker) package used by the face tracking example outputs so-called blendshapes signals, which provide information on various aspects of the face expression. These signals are logged under the `/blendshapes` root entity by the face tracking example.

One signal, `jawOpen` (logged under the `/blendshapes/0/jawOpen` entity as a [`Scalar`](../../reference/types/components/scalar.md) component), is of particular interest for our purpose. Let's inspect it further using a timeseries view:

<picture>
  <img src="https://static.rerun.io/data-out-jaw-open-signal/258f5ffe043b8affcc54d5ea1bc864efe7403f2c/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/data-out-jaw-open-signal/258f5ffe043b8affcc54d5ea1bc864efe7403f2c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/data-out-jaw-open-signal/258f5ffe043b8affcc54d5ea1bc864efe7403f2c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/data-out-jaw-open-signal/258f5ffe043b8affcc54d5ea1bc864efe7403f2c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/data-out-jaw-open-signal/258f5ffe043b8affcc54d5ea1bc864efe7403f2c/1200w.png">
</picture>

This signal indeed seems to jump from approximately 0.0 to 0.5 whenever the jaws are open. We also notice a discontinuity in the middle of the recording. This is due to the blendshapes being [`Clear`](../../reference/types/archetypes/clear.md)ed when no face is detected.

Let's create a dataframe view to further inspect the data:

<picture>
  <img src="https://static.rerun.io/data-out-jaw-open-dataframe/bde18eb7b159e3ea1166a61e4a334eaedf2e04f8/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/data-out-jaw-open-dataframe/bde18eb7b159e3ea1166a61e4a334eaedf2e04f8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/data-out-jaw-open-dataframe/bde18eb7b159e3ea1166a61e4a334eaedf2e04f8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/data-out-jaw-open-dataframe/bde18eb7b159e3ea1166a61e4a334eaedf2e04f8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/data-out-jaw-open-dataframe/bde18eb7b159e3ea1166a61e4a334eaedf2e04f8/1200w.png">
</picture>

Here is how this view is configured:

-   Its content is set to `/blendshapes/0/jawOpen`. As a result, the table only contains columns pertaining to that entity (along with any timeline(s)). For this entity, a single column exists in the table, corresponding to entity's single component (a `Scalar`).
-   The `frame_nr` timeline is used as index for the table. This means that the table will contain one row for each distinct value of `frame_nr` for which data is available.
-   The rows can further be filtered by time range. In this case, we keep the default "infinite" boundaries, so no filtering is applied.
-   The dataframe view has other advanced features which we are not using here, including filtering rows based on the existence of data for a given column, or filling empty cells with latest-at data.

Now, let's look at the actual data as represented in the above screenshot. At around frame #140, the jaws are open, and, accordingly, the `jawOpen` signal has values around 0.55. Shortly after, they close again and the signal decreases to below 0.1. Then, the signal becomes empty. This happens in rows corresponding to the period of time when the face cannot be tracked and all the signals are cleared.

## Next steps

Our exploration of the data in the viewer so far provided us with two important pieces of information useful to implement the jaw open detector.

First, we identified that the `Scalar` value contained in `/blendshapes/0/jawOpen` contains relevant data. In particular, thresholding this signal with a value of 0.15 should provide us with a closed/opened jaw state binary indicator.

Then, we explored the numerical data in a dataframe view. Importantly, the way we configured this view for our needs informs us on how to query the recording from code such as to obtain the correct output.

<!-- TODO(#7462): improve the previous paragraph to mention copy-as-code instead -->

From there, our next step is to query the recording and extract the data as a Pandas dataframe in Python. This is covered in the [next section](export-dataframe.md) of this guide.
