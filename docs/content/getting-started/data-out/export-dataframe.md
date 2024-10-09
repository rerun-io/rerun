---
title: Export the dataframe
order: 2
---

**OVERVIEW**
- save rrd and use "copy-as-code" to load the data in pandas
- explore the dataframe schema and content
- explain:
  - NaN index (-> static) 
  - column names
  - sub-array thing (`values.flatten()`)
- "[Next](analyze-and-log): analyze the dataframe to detect mouth open and log back the results"


<hr/>

## Load the recording

- Save the rrd from the viewer.
- In a new python script, load the RRD:

```python
import rerun as rr


def main():
    # TODO: loading directly from the viewer would be sweet
    recording = rr.dataframe.load_recording("face_tracking.rrd")

```

## Extract data

- Copy-as-code from the dataframe view and past the function in the code
- Flatten the `Scalar` column

  TODO: this step is annoying, either smooth it away somehow, or link to some material that explains why data is nested in inner arrays


```python
import rerun as rr
import pandas as pd

SIGNAL_COLUMN = "/blendshapes/0/jawOpen:Scalar"

# copied from the dataframe view
def export_to_pandas(recording: rr.dataframe.Recording) -> pd.DataFrame:
    return (
        recording.view(index="frame_nr", contents={"/blendshapes/0/jawOpen")
        .select(
            rr.dataframe.IndexColumnSelector("frame_nr"),
            rr.dataframe.ComponentColumnSelector(
                "/blendshapes/0/jawOpen",
                "rerun.components.Scalar",
            ),
        )
        .read_pandas()
    )

def load_mouth_open_data(recording: rr.dataframe.Recording) -> pd.DataFrame:
    df = export_to_pandas(recording)
    # TODO: how can we do that better?
    df[SIGNAL_COLUMN] = df[SIGNAL_COLUMN].values.flatten()
    # df[SIGNAL_COLUMN] = df[SIGNAL_COLUMN].apply(lambda x: x[0] if x is not None else x)  # alternative to handle null (slower?)

    return df


def main():
    # Load the recording
    # TODO: loading directly from the viewer would be sweet
    recording = rr.dataframe.load_recording("face_tracking.rrd")

    # Extract the data
    df = load_mouth_open_data(recording)
```


## Explore the data

TODO


## Complete script

TODO