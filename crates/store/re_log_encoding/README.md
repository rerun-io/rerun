# re_log_encoding

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_log_encoding.svg)](https://crates.io/crates/re_log_encoding)
[![Documentation](https://docs.rs/re_log_encoding/badge.svg)](https://docs.rs/re_log_encoding)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Helper library for encoding Rerun log messages.

```
Name
./
├── benches/
│   └── msg_encode_benchmark.rs # transport and app-level encoding/decoding benchmarks for the bare naked Encoder/Decoder impls
├── tests/
│   └── arrow_encode_roundtrip.rs # a random test that lives on its own, for whatever reason 🤷
├── src/
│   ├── codec/
│   │   ├── file/
│   │   │   ├── decoder.rs  # helpers to decode LogMsg from protobuf bytes
│   │   │   ├── encoder.rs  # helpers to encode LogMsg to protobuf bytes
│   │   │   └── mod.rs  # contains MessageHeader. for some reason FileHeader and StreamHeader are not here. they should be
│   │   ├── wire/
│   │   │   ├── decoder.rs  # some weird traitext helpers to deserialize a RecordBatch from protobuf, it's weird af
│   │   │   ├── encoder.rs  # some weird traitext helpers to serialize a RecordBatch to protobuf, it's weird af
│   │   │   └── mod.rs  # just a random test (???)
│   │   ├── arrow.rs  # it's just a bunch of unrelated IPC serialization helpers (??)
│   │   └── mod.rs  # nothing beyond CodecError
│   ├── decoder/
│   │   ├── mod.rs  # there is one of 3 decoders in here, apparently used by the CLI and data loaders
│   │   ├── stream.rs  # another sync decoder, which seems of much higher quality, but only used for http for some reason?
│   │   └── streaming.rs  # an async decoder, with a very different feature, used heavily by redap
│   ├── app_id_injector.rs  # the hacky decoding middleware machinery to retrofill app-ids for legacy data. used by all decoders.
│   ├── encoder.rs  # an actual encoder. nothing too weird except multiple types and free-floating functions that are annoying.
│   ├── file_sink.rs  # a file sink impl that's hardcoded to use the DroppableEncoder
│   ├── lib.rs  # for some reason there is so much shit that's specific to file encoding in there
│   ├── protobuf_conversions.rs  # just a bunch of proto extensions that for some reason live here
│   └── stream_rrd_from_http.rs  # somewhat leaky abstractions and helpers for HTTP loading on native and wasm. use StreamDecoder.
├── Cargo.toml
└── README.md
```
