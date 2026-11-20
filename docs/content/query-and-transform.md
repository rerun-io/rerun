---
title: Query and Transform
order: 5
description: The catalog, dataframe queries, and reshaping data with lenses
index: true
---

The Rerun SDK connects to a catalog server, which allows you to store, retrieve, and query over large amounts of data, and integrates with the SDK so you can browse and inspect the data visually.

The Rerun SDK includes a simplified open-source catalog server that is API compatible with [Rerun Hub](hub.md), our commercial managed offering for production deployments. <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->
The examples in this section use the open-source server so that you can run them locally, and they also run against Hub (see [Run the examples on Hub](hub.md#run-the-examples-on-hub)).
See [The open-source server](query-and-transform/catalog/oss-server.md) for how it stores data and serves datasets larger than RAM.

New to querying?
Start with the [Query and Transform tutorial](query-and-transform/tutorial.md), or see [Query data out of Rerun](query-and-transform/catalog/get-data-out.md) for launching and connecting to the server.
