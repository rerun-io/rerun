---
title: Migrating from 0.23 to 0.24
order: 986
---
<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Changed timeline navigation keyboard shortcut

To accommodate the new tree keyboard navigation feature, the timeline navigation is changed as follows:

- go to previous/next frame is ctrl-left/right (cmd on Mac) arrow (previously no modifier was needed)
- go to beginning/end of timeline is alt-left/right (previously the ctrl/cmd modifier was used)

## Previously deprecated, now removed

### `Scalar`, `SeriesLine`, `SeriesPoint` archetypes

Have been removed in favor of `Scalars`, `SeriesLines`, `SeriesPoints` respectively.
