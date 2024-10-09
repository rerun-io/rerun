---
title: Explore a recording with the dataframe view
order: 1
---

**OVERVIEW**
- introduce RRD for this series (-> face tracking)
- create a dataframe view with all the data
- explain column meaning
- do not explain latest at & pov -> link to something else 
- "[Next](export-dataframe): export the dataframe to Pandas"


<hr>

TODO: intro

## Create some data

For this tutorial, we use the [face tracking example](https://rerun.io/examples/video-image/face_tracking).

### Installation

See the [face tracking installation instruction](https://rerun.io/examples/video-image/face_tracking#run-the-code)


### Acquiring data

Run the example and open/close the mouth. This creates an interesting signal: `/blendshapes/0/jawOpen`.

Bonus: temporarily hide the camera to temporarily [`Clear`](../reference/types/archetypes/clear.md) the signal.

TODO:
- add video screenshot of rerun
- add a live viewer pointed at a saved RRD


### Display the data of interest

- Setup a time series view of the signal
- Setup a dataframe view of the signal
- Explore the data => a threshold at 0.15 should do the trick

![bp](https://i.postimg.cc/mrSPkRTg/image.png)

TODO: proper screenshot
