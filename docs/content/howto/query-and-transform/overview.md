---
title: Overview
order: 10
description: A quick tour of the SDK and catalog server
---

Rerun is the Unified Data Layer for Physical AI.
The Rerun SDK connects to a catalog server, which allows you to store, retrieve, and query over large amounts of data, and integrates with the SDK so you can browse and inspect the data visually.

The Rerun SDK includes a simplified open-source catalog server that is API compatible with Rerun Hub, our managed offering.
The open-source server keeps catalog metadata in memory, but reads data from RRDs with a footer on demand instead of loading the entire recording.
This allows it to serve datasets larger than RAM.
RRDs without a footer are loaded into memory; use [`rerun rrd optimize`](../../reference/cli.md#rerun-rrd-optimize) to add a footer before serving them.

See the [how-to guide for the open-source server](get-data-out.md) for more details on launching and connecting to the server.
