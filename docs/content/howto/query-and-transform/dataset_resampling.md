---
title: Dataset Resampling
order: 110
---

This snippet demonstrates how to resample a dataset based on the time index of one component
within your data. This is particularly helpful when you have data that is produced at very
different frequencies.

First, load a dataset to use for evaluation:

snippet: howto/dataset_resampling[get_dataset]

## Investigate time ranges

Before we do the resampling, we can examine the dataset's time ranges using the function
`get_index_ranges()`. This is not strictly necessary for the resampling work to follow, but
it can be helpful during investigation of your data. This will show you the start and
end values for all indexes in your dataset, one per segment.

snippet: howto/dataset_resampling[view_index_ranges]

## Prior to resampling

The sample data we have loaded is very basic, but it demonstrates having components from
three different entities at different times in the dataset. The code below demonstrates
what the data looks like before resampling. In order to do data analysis on this DataFrame
you would likely need to do some aggregation or window across the time index.

snippet: howto/dataset_resampling[original_data]

## Resampled data

The snippet below demonstrates resampling using two lines. First we create a new DataFrame
which contains the index values we care about per segment. It is *very* important in
doing this that you do not set `fill_latest_at=True`. Otherwise it would negate the effect
we are trying to produce where we only have rows for which we have data in our component
of interest. The required output of this DataFrame is only the segment ID and the index
value.

Once we have a DataFrame with these index values, we can now query the dataset using that
DataFrame. You can see from the output below that we generate one row per time index for
which the component of interest is not null.

snippet: howto/dataset_resampling[resampled_data]
