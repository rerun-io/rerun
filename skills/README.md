# Rerun agent skills

Skills that teach a coding agent how to use Rerun: how to model a dataset, how to ingest one, how to lay it out in the viewer, etc.
Each skill is one directory holding a `SKILL.md`, plus optional helper scripts.

Install them into your own project with:

```sh
npx skills add rerun-io/rerun
```

The Rerun Viewer's agent panel embeds this directory into the binary, so its agent gets the same skills with no install.

## The skills

Ingestion is ordered: decide the data model first, then the mechanism, then read the skill for your source format.

| Skill                                                       | What it covers                                                                         |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| [`rerun-data-model`](rerun-data-model/SKILL.md)             | **Read first.** Entity vs component, property vs layer, static vs temporal             |
| [`rerun-chunk-processing`](rerun-chunk-processing/SKILL.md) | The mechanism: `LazyChunkStream`, `Chunk`, lenses, `RrdReader`, writing optimized RRDs |
| [`rerun-mcap`](rerun-mcap/SKILL.md)                         | `McapReader`: topics, decoders, custom protobuf                                        |
| [`rerun-parquet`](rerun-parquet/SKILL.md)                   | `ParquetReader`: column grouping, index and static columns                             |
| [`rerun-mp4`](rerun-mp4/SKILL.md)                           | `Mp4Reader`: stream vs asset mode, transcoding, PTS alignment                          |
| [`rerun-urdf`](rerun-urdf/SKILL.md)                         | `UrdfTree`: static model plus forward kinematics as a transform layer                  |
| [`rerun-lerobot`](rerun-lerobot/SKILL.md)                   | LeRobot datasets: the built-in directory importer, per-episode splitting               |

Standalone:

| Skill                                                     | What it covers                                                        |
| --------------------------------------------------------- | --------------------------------------------------------------------- |
| [`rerun-docs`](rerun-docs/SKILL.md)                       | Where the docs and code snippets live on disk, and how to search them |
| [`rerun-blueprint`](rerun-blueprint/SKILL.md)             | Designing a layout, then iterating on it from headless screenshots    |
| [`rerun-catalog-queries`](rerun-catalog-queries/SKILL.md) | Performance patterns for querying a catalog from Python               |

Internal, not aimed at users of the SDK:

| Skill                                               | What it covers                                                                       |
| --------------------------------------------------- | ------------------------------------------------------------------------------------ |
| [`assemble-changelog`](assemble-changelog/SKILL.md) | Turns `docs/content/changelog/upcoming/` into a release changeset and `CHANGELOG.md` |
