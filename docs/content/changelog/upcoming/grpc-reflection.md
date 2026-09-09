---
title: gRPC server reflection
hidden: true
type: feature
---

### gRPC server reflection

Every Rerun gRPC server now serves [gRPC server reflection](https://grpc.io/docs/guides/reflection/): the SDK proxy, the Viewer, and `rerun server`.
Generic gRPC tooling can discover the Rerun services and their message types without the `.proto` files:

```sh
grpcurl -plaintext 127.0.0.1:9876 list
grpcurl -plaintext 127.0.0.1:9876 describe rerun.cloud.v1alpha1.RerunCloudService
```

See the [gRPC API reference](../reference/grpc.md).
