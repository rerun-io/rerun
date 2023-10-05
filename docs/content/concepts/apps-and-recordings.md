---
title: Application IDs and Recording IDs
order: 8
---

## Application ID
When you initialize rerun with [`rr.init`](https://ref.rerun.io/docs/python/nightly/common/initialization_functions/#rerun.init) you need to set an Application ID.

Your Rerun Viewer will store the Blueprint based on this Application ID.
This means that you can run your app and set up the viewer to your liking,
and then when you run the app again the Rerun Viewer will remember how you set up your Space Views etc.

## Recording ID
Each time you start logging using Rerun, a random _Recording ID_ is generated.
For instance, each `.rrd` file will have a unique Recording ID.

This means you can have multiple recordings with different Recording IDs sharing the same application ID.

If you want to log from multiple processes and want all the log data to show up
together in the viewer, you need to make sure all processes use the same Recording ID.
