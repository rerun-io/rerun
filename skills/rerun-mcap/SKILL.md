---
name: rerun-mcap
description: Ingest MCAP files into Rerun chunk streams with rerun.experimental.McapReader. Read when converting an MCAP recording, selecting topics or decoders, decoding custom protobuf messages, or when an MCAP-derived stream comes out empty. Builds on rerun-chunk-processing (stream mechanics) and rerun-data-model (what the topics should become).
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun MCAP ingestion

`McapReader` turns an MCAP file into a lazy chunk stream: one entity per topic
at the topic's path, message payloads decoded by pluggable decoders. This
skill covers the reader's options, what each topic becomes, and the failure
modes that yield an empty stream with no error. Stream mechanics (filter, drop,
lenses, merge, write) are in `rerun-chunk-processing`.

## The API

```python
from rerun.experimental import McapReader

reader = McapReader(mcap_path)  # see help(McapReader) for the full option set
stream = reader.stream()
```

A URDF embedded in the MCAP can be ingested as well (then see `rerun-urdf`).

## What a topic becomes

- Entity path = topic name (`/sensors/joint_states` stays `/sensors/joint_states`).
  Filter early: `McapReader(path).stream().filter(content="/sensors/**")`.
- A reflection-decoded message lands as one struct component named `<fully.qualified.MessageName>:message`.
  Navigate it with `Selector` (`Selector(".joint_positions")`) inside lenses; this is how custom messages
  get Rerun semantics attached (see the DeriveLens patterns in `rerun-chunk-processing`).
- Topic regexes use RE2 syntax and are **not anchored**: `cam` matches
  `/external/cam_low` and `/camera_info`.
  Anchor explicitly (`^/external/cam`) when it matters. Prefer reader-level topic filtering over `.filter(...)`
  when you can, so excluded topics are never decoded at all.

## When to use the low-level `mcap` package instead

`McapReader` keeps payloads in columnar chunk streams; that is almost always what you want.
Drop to `mcap.reader.make_reader` only when you need raw record metadata without payloads, or when you need to rewrite the container itself (re-registering schemas, channels, and messages).

## Gotchas

1. Empty stream, no error: a topic regex that matched nothing, or a channel
   whose decoder produced no rows. Check `Chunk.format()` on a few chunks of
   `reader.stream().to_chunks()` against a tiny test file, or compare topic
   names with the `mcap` CLI / package first.
2. Topic regexes are unanchored RE2; excludes run after includes.
3. `timeline_type="timestamp"` interprets MCAP log times as wall-clock ns
   since epoch. If the recording's clock is wrong, fix it at the reader with
   `timestamp_offset_ns` rather than mutating timestamps downstream.
4. Decoder subsets silently skip topics no decoder claims; when a topic is
   missing, retry with `decoders=None` to rule out decoder selection.

## References

- End-to-end MCAP pipeline: `https://github.com/rerun-io/rerun/tree/main/examples/python/robot_data_preprocessing`
- `rerun-chunk-processing` (stream/lens mechanics), `rerun-urdf` (FK from joint-state topics), `rerun-data-model` (modeling decisions)
