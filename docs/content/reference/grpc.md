---
title: gRPC API
order: 925
---

Rerun components talk to each other over [gRPC](https://grpc.io/).
The protocol is defined by the `.proto` files in [`crates/store/re_protos/proto`](https://github.com/rerun-io/rerun/tree/main/crates/store/re_protos/proto), and every Rerun server also describes itself through [gRPC server reflection](https://grpc.io/docs/guides/reflection/).

## Servers and services

| Server                           | Default address   | Services                                                           |
| -------------------------------- | ----------------- | ------------------------------------------------------------------ |
| Viewer (`rerun`)                 | `127.0.0.1:9876`  | `MessageProxyService`, `ViewerControlService`, `RerunCloudService` |
| SDK proxy (`rerun --serve-grpc`) | `127.0.0.1:9876`  | `MessageProxyService`                                              |
| Catalog server (`rerun server`)  | `127.0.0.1:51234` | `RerunCloudService`                                                |

`MessageProxyService` carries logged data between SDKs and Viewers.
`ViewerControlService` drives a running Viewer, and is what the [MCP server](viewer/mcp.md) uses.
`RerunCloudService` is the catalog and query API that `rerun.catalog.CatalogClient` speaks.
It is served by two different catalogs:

* The Viewer serves its *internal catalog*: the recordings currently loaded in that Viewer.
  It only accepts connections from the local machine, like `ViewerControlService`.
* `rerun server` serves a catalog of the `.rrd` files and tables it was started with, for any client that can reach it.

## Server reflection

Reflection lets generic gRPC tooling discover the services and message types of a live server, without access to the `.proto` files.
Both the `v1` and the `v1alpha` reflection protocols are served.

Using [`grpcurl`](https://github.com/fullstorydev/grpcurl):

```sh
# Which services does this address speak?
grpcurl -plaintext 127.0.0.1:9876 list

# Describe a service, or a message type:
grpcurl -plaintext 127.0.0.1:9876 describe rerun.cloud.v1alpha1.RerunCloudService
grpcurl -plaintext 127.0.0.1:9876 describe rerun.cloud.v1alpha1.FindEntriesRequest

# Call a method:
grpcurl -plaintext -d '{}' 127.0.0.1:9876 rerun.cloud.v1alpha1.RerunCloudService/FindEntries
```

`list` names only the services the address serves.
`describe` knows the whole Rerun protocol, including message types of services the address does not serve.
