Requires the Protobuf Compiler `protoc` to be installed:

* Linux: `apt install -y protobuf-compiler`
* Mac: `brew install protobuf`
* Or visit <https://grpc.io/docs/protoc-installation/>

It would be nice to use [`protoc_prebuilt`](https://crates.io/crates/protoc-prebuilt) here, but [it has a huge dependency tree](https://github.com/sergeiivankov/protoc-prebuilt/issues/1).
