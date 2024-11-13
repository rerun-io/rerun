

Notes on rethinking RRD streams' encodings and transport layers
===============================================================

We'd like to slowly but surely..
* ..move away from custom encodings and towards Protobuf for everything instead.
* ..move away from custom transports and towards gRPC for everything instead.

This unification will allow all pieces of the Rerun ecosystem to communicate bidirectionally with one another, in a standard way, starting from files and all the way up to the dataplatform itself.

The two topics (i.e. Protobuf & gRPC) are obviously closely related, although most of the work can in fact happen concurrently.

This document is a just a hodgepodge of notes covering what we have now and some of the subtleties to keep in mind as we move towards that unification.


TODO: considerations
TODO: RRD streams at rest (RRD files) vs. RRD streams in flight


## State of affairs

_Not accounting for the on-going dataplatform related efforts!_

### Overview

Everything in Rerun is encoded the same way, whether it's:
* an SDK pushing chunks to a viewer/server via TCP
* an SDK pushing chunks to a file
* a viewer pulling chunks from another viewer/server via WebSocket
* other (?)

(Somewhat related, we now have a bit of documentation regarding all these possible workflows: [click here](https://github.com/rerun-io/rerun/blob/main/docs/content/concepts/app-model.md).)

The data is modeled as a stream of RPC commands (specifically, three of them: `SetStoreInfo`, `ArrowMsg`, and `BlueprintActivationCommand`) -- a so called RRD stream.

Each command is wrapped in a `LogMsg`, which is then serialized using `serde`'s MsgPack backend:
https://github.com/rerun-io/rerun/blob/db9033457305a6c4dc7b2d7814468a54367fd786/crates/store/re_log_types/src/lib.rs#L257-L278
```rust
pub enum LogMsg {
    SetStoreInfo(SetStoreInfo),
    ArrowMsg(StoreId, ArrowMsg),
    BlueprintActivationCommand(BlueprintActivationCommand),
}
```

<!-- TODO: remove this -->
<!-- NOTE: I will refer to `LogMsg`'s `ArrowMsg` as `WriteChunk` going forward. It what it actually means in practice, and it will make the rest of this document clearer going forward. -->

Place these serialized `LogMsg`s one after the other (whether it's in memory, in a file, in a socket, or wherever), prepend some magic bytes and file-level headers, add some headers for each message, and you've got yourself a Rerun RRD stream:
```
<FileHeader { magic bytes, version, compression }>
<MessageHeader { raw_len, compressed_len }>
<SerializedLogMsg>
<MessageHeader { raw_len, compressed_len }>
<SerializedLogMsg>
<MessageHeader { raw_len, compressed_len }>
<SerializedLogMsg>
...
```

TODO: if the file header is fixed-sized, one can update it repeatedly, even in a streaming context

You can now forward these stream to a file or a viewer however you wish, and it just works.

All in all it's pretty nice, the only real issue is the lack of standardization: we need all these things to play nicer with common transports and encodings, starting with our own dataplatform.

**Action points**:
* Custom Rust types serialized via `serde`/MsgPack should be replaced by Protobuf messages.
* Custom transports over raw TCP & WebSocket should be replaced in favor of gRPC.


### Multiplexing

Every RRD stream is assumed to be multiplexed: an RRD stream can always contain data for any number of recordings (i.e. datastores). That includes RRD files since, they too, are just RRD streams.
How this works in practice is that every single message in an RRD stream carries its `StoreId`. The viewer uses these `StoreId`s to dispatch the data to the right place (`SmartChannel`s).

We would like to move away from that, as it has proven to be a PITA more often than not:
* https://github.com/rerun-io/rerun/issues/7927

The only situation where this is absolutely required today is the web viewer: the web viewer connects to a single WebSocket server and expects to be served every recording available on that one WebSocket stream.
The migration of the web-viewer over to gRPC-web would be a good opportunity to fix that. E.g. make the WebSocket server behave more like a dataplatform: the web-viewer connects to it using a single gRPC URI and then can subsribe to any recording in the recording from there on.

**Action point**: RRD streams should *not* be multiplexed. We should design our new transports and encodings with that fact in mind.


### Compression

RRD streams support compression at the stream-level: the `FileHeader` specifies the compression algorithm, and every single message in the stream must follow.

This is very nice, although perhaps a little overkill in practice? Only the `ArrowMsg` messages really have a need for compression, everything else is pretty small to start with.

**Action point**: Decide whether we want to keep compression support at the stream layer, or move it inwards. There are pros and cons to both cases, although I'm definitely leaning towards moving it inwards, as that would make every message interpretable without any further context.


### `ArrowMsg`, Chunks, and their metadata

`ArrowMsg` is just a blob of bytes as far as MsgPack is concerned -- the bytes themselves correspond to a Rerun Chunk, previously serialized according to the Arrow IPC standard.

Within that Arrow blob, we not only find the actual data for a Rerun Chunk, but also the chunk-level and column-level metadata, encoded directly as string tuples into the Arrow blob.
Not having a proper container for these pieces of metadata in a real problem in practice, see:
* https://github.com/rerun-io/rerun/issues/6572

**Action point**: We need a Protobuf message that holds Chunk metadata information in a typed and efficient way. Emphasis on efficient: metadata is used all over the place to dispatch and index the data as it comes in.


### Ordering & idempotency

Technically, RRD streams are both order-dependent and non-idempotent, by virtue of them just being A) streams and B) series of RPC commands that have side-effects.
In practice, it's a bit more subtle than that because the underlying datastore is OTOH both order-independent and idempotent...

Looking closer at our 3 existing commands..:
* `SetStoreInfo` is both order-dependent and non-idempotent: the viewer will settle on the last `StoreInfo` set for each individual `StoreId` in the RRD stream.
* `ArrowMsg` is both order-independent and idempotent: you can randomly shuffle all `ArrowMsg`s in any RRD stream and that stream will be semantically identical to the original one, because such are the semantics of the datastore itself.
* `BlueprintActivationCommand` is both order-dependent and non-idempotent, for similar reasons as `SetStoreInfo`.

Put differently, RRD streams are always fully ordered and you cannot mess with that order without impacting semantics... but our datastore is unordered, and therefore in practice `ArrowMsg` is unordered.

**Action point**: This all feels very brittle:
* Should we really be mixing commands with side-effects into an otherwise pure data stream?
* Should `SetStoreInfo` really be a command of its own, or could that just be part of the stream-level header or something of that nature?
* Naming could certainly be improved.


### Backwards compatibility

TODO: this is not backwards compatibility, this is breaking changes during the transition.

Even if you leave the transport layer out of the picture, there are many different layers of backwards compatibility for an RRD stream:
* Stream encoding: `serde`/MsgPack vs. Protobuf
* Chunk metadata encoding: RecordBatch map<string> metadata with Arrow IPC vs. Protobuf wrapper
* Chunk encoding: why would you use anything but Arrow IPC?
* Chunk topology: what are the columns, how are they named, etc.
* Components encodings: how are each individual components encoded, etc.

We're only interested in things happening at the stream-level in this document, so anything regarding how individual Chunks or even components are encoded is irrelevant for now.
That leaves the stream level and chunk metadata encodings, both of which could heavily benefit from Protobuf's backwards and forwards compatibility guarantees.

**Action point**: 


### Internal Chunks vs. external RecordBatches (kinda out of scope)

TODO: Either way, not relevant for the problem at hand.
TODO: internal chunks (`Chunk`) vs. external chunks (Sorbet, dataframes, record batches, etc)
TODO: Chunks used internally vs. what the dataframe APIs return.
TODO: should internal chunks 


**Action points**: 
* Should internal and external chunks be one and the same?


### Others?

I'm likely missing a lot -- contributions welcome.


## Where we're going

_Short to mid term._

* Short-term goal is to standardize everything by migrating all the different pieces to Protobuf & gRPC without really changing any of the semantics.
* Mid-term goal is to evolve the semantics to fix some long-standing issues.


### Encoding

Encoding-wise, all we'd like to achieve for now is to get rid of the non-standardized parts of the pipeline, namely replace the custom `serde` objects and MsgPack serialization with Protobuf definitions.
The semantics stay the same: an RRD stream is still a series of messages where each message corresponds to one of our RPC commands.

We'll need to define new Protobuf messages for our three RPC commands (`SetStoreInfo`, `WriteChunk` (used to be `ArrowMsg`), `ActivateBlueprint` (used to be `BlueprintActivationCommand`)).
In addition to those, we will also need a generic `ArrowChunk` message that will be used all over the place (anytime we need to carry Arrow data around, which is a lot).

```proto
// TODO: should this contain statistics? why would it?
message ArrowChunkMetadata {
    repeated ArrowChunkMetadata columns = 10;
}

message ArrowChunkColumnMetadata {
    // TODO: look at what we have today
}

message ArrowChunk {
    ArrowChunkMetadata metadata = 0;

    /// Arrow-IPC encoded Arrow schema.
    bytes arrow_schema = 1;

    /// Arrow-IPC encoded Arrow data (also holds metadata in the form of string tuples as of today).
    bytes arrow_data   = 2;
}
```

```proto
enum Compression {
    NONE = 0,
    GZIP = 1,
    LZ4  = 2,
}

// TODO: only for "raw" (i.e. not gRPC) streams
// TODO: need to be able to read this with a fixed-sized buffer
message MessageHeader {
    Compression compression;
    fixed32     raw_size_bytes;
    fixed32     compressed_size_bytes;
}
```

```proto
message StoreInfo {
    // a lot of stuff, doesn't really matter for this discussion
}

message SetStoreInfo {
    StoreInfo info = 0;
}

message WriteArrowChunk {

}

message ActivateBlueprint {

}
```


#### At rest

TODO: We need some magic header proto message

#### In flight

TODO: gRPC has its own magic headers






Each command is wrapped in a `LogMsg`, which is then serialized using `serde`'s MsgPack backend:
https://github.com/rerun-io/rerun/blob/db9033457305a6c4dc7b2d7814468a54367fd786/crates/store/re_log_types/src/lib.rs#L257-L278
```rust
pub enum LogMsg {
    SetStoreInfo(SetStoreInfo),
    ArrowMsg(StoreId, ArrowMsg),
    BlueprintActivationCommand(BlueprintActivationCommand),
}
```

NOTE: I will refer to `LogMsg`'s `ArrowMsg` as `WriteChunk` going forward. It what it actually means in practice, and it will make the rest of this document clearer going forward.

Place these serialized `LogMsg`s one after the other (whether it's in memory, in a file, in a socket, or wherever), prepend some magic bytes, add some headers for each message, and you've got yourself a Rerun RRD stream:
```
<FileHeader { magic bytes, version, compression }>
<MessageHeader { raw_len, compressed_len }>
<SerializedLogMsg>
<MessageHeader { raw_len, compressed_len }>
<SerializedLogMsg>
<MessageHeader { raw_len, compressed_len }>
<SerializedLogMsg>
...
```

You can now forward these stream to a file or a viewer however you wish, and it just works.

All in all it's pretty nice, the only real issue is the lack of standardization: we need all these things to play nicer with common transports and encodings, starting with our own dataplatform.


#### Aside: multiplexing

Every RRD stream is assumed to be multiplexed: an RRD stream can always contain data for any number of recordings (i.e. datastores). That includes RRD files since, they too, are just RRD streams.

We would like to move away from that, as it has proven to be a PITA more often than not:
* https://github.com/rerun-io/rerun/issues/7927

The only situation where this is absolutely required today is the web viewer: the web viewer connects to a single WebSocket server and expects to be served every recording available on that one WebSocket stream.
We can easily change that while porting the web viewer to gRPC, so that issue goes away.

TODO: in the short-term we want to keep multiplexing alive if only for bw...


#### Aside: compression

RRD streams support compression at the stream-level: the `FileHeader` specifies the compression algorithm, and every single message in the stream must follow.

This is very nice, although it might be a little overkill in practice: only the `ArrowMsg` messages really have a need for compression.


#### Aside: `ArrowMsg`, chunks, and chunk metadata

`ArrowMsg` is just a blob of bytes as far as MsgPack is concerned -- the bytes themselves correspond to a Rerun Chunk, serialized following the Arrow IPC standard.

Within that Arrow blob, we not only find the actual data for a Rerun columnar data, but also the chunk and 


#### Aside: ordering & idempotency

Technically, RRD streams are order-dependent and non-idempotent, by virtue of them just being a series of RPC commands with side-effects.
In practice, it's a bit more subtle than that because the underlying datastore is both order-independent and idempotent...

Semantically, RRD streams are both ordered and unordered, it depends on the kind of message you're looking at :grimacing:.
Looking closer at our 3 existing commands, and focusing on order dependency:
* `SetStoreInfo` messages are ordered (..IIRC?): the viewer will settle on the last `StoreInfo` set for each individual `StoreId` in the RRD stream.
* `ArrowMsg` messages are unordered: you can randomly shuffle all `ArrowMsg`s in any RRD stream and that stream will be semantically identical to the original one.
* `ActivateBlueprint` messages are obviously very ordered.

Put differently, RRD streams are always fully ordered and you cannot mess with that order without impacting semantics... but our datastore is unordered, and therefore in practice `ArrowMsg` is unordered.

Idempotency is roughly the same story: 
* `SetStoreInfo` is generally idempotent, as long as you keep sending the same store infos.
* `ArrowMsg` is idempotent because the datastore itself is idempotent.
* `ActivateBlueprint` is a pure command so idempotency doesn't really make sense.




### Transport


TODO: mention copy pasting proto definitions, it's idiomatic!!
TODO: `LogMsg` keeps existing for now.
TODO: gRPC has its own headers
TODO: writing chunks (TCP, store-multiplexed, unidirectional)
TODO: TransportChunk
TODO: should RRDs be multiplexed
TODO: should WriteChunks (TBD) be multiplexed?
TODO: some notes about sharing, copy-pasting, and generally evolving Protobuf definitions
TODO: talk about grpc stream metadata
TODO: backwards compat
TODO: rrd conversion tool
TODO: definitely mention https://github.com/rerun-io/rerun/issues/7988 somewhere ("Replace our serde/MsgPack encoding of `enum LogMsg` with Protobuf")
TODO: might want to mention that we allow StoreInfos in any order, and we might want to get them in another stream in the future
TODO: mention https://github.com/rerun-io/rerun/issues/6574 ("`StoreId` should be passed as Chunk metadata")
