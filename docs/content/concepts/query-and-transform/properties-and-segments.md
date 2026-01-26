---
title: Properties and segment tables
order: 200
---

Properties are recording-level metadata that relates to an entire segment.
When you query a [dataset](catalog-object-model.md#datasets), properties appear as columns in the segment table and can be used to filter, sort, and analyze your segments.
Common use cases for properties include tagging recordings with capture location, data format version, environmental conditions, or any other custom metadata relevant to your workflow.

## Understanding properties

Let's use an example to illustrate how properties work and how they can be retrieved and queried using the Rerun Data Platform.

First, we create a few recordings with some properties:

snippet: concepts/query-and-transform/segment_properties[setup]

In this example, we use `send_property()` to attach metadata to each recording. Properties are regular Rerun data, so you can use any built-in archetype. Here we use [`GeoPoints`](../../reference/types/archetypes/geo_points.md) to store a geographic location. For arbitrary data that doesn't fit an existing archetype, use [`AnyValues`](../../howto/logging-and-ingestion/custom-data.md).

In addition to user-provided properties, Rerun automatically stores built-in properties using the [`RecordingInfo`](../../reference/types/archetypes/recording_info.md) archetype.
Its `start_time` field is automatically populated, and its `name` field can be set with `send_recording_name()`.

Internally, properties are logged under a reserved `/__properties` entity path and use [static semantics](../logging-and-ingestion/static.md) since they apply to the entire recording rather than specific points in time.

## Querying the segment table

Once recordings are registered to a [dataset](catalog-object-model.md#datasets), their properties become visible and queryable through the segment table.
Here we use the local open-source Data Platform included with Rerun to illustrate this:

snippet: concepts/query-and-transform/segment_properties[segment_table]


Output:

```
┌──────────────────────────────────┬────────────────────────────────────┬─────────────────────────────────────┬───────────────────────────────────┬────────────────────────────────────┬──────────────────────────────────────────────────────────────┐
│ rerun_segment_id                 ┆ property:RecordingInfo:name        ┆ property:RecordingInfo:start_time   ┆ property:info:index               ┆ property:info:is_odd               ┆ property:location:GeoPoints:positions                        │
│ ---                              ┆ ---                                ┆ ---                                 ┆ ---                               ┆ ---                                ┆ ---                                                          │
│ type: Utf8                       ┆ type: nullable List[nullable Utf8] ┆ type: nullable List[nullable i64]   ┆ type: nullable List[nullable i64] ┆ type: nullable List[nullable bool] ┆ type: nullable List[nullable FixedSizeList[nullable f64; 2]] │
│                                  ┆ archetype: RecordingInfo           ┆ archetype: RecordingInfo            ┆ component: index                  ┆ component: is_odd                  ┆ archetype: GeoPoints                                         │
│                                  ┆ component: RecordingInfo:name      ┆ component: RecordingInfo:start_time ┆ entity_path: /__properties/info   ┆ entity_path: /__properties/info    ┆ component: GeoPoints:positions                               │
│                                  ┆ component_type: Name               ┆ component_type: Timestamp           ┆ kind: data                        ┆ kind: data                         ┆ component_type: LatLon                                       │
│                                  ┆ entity_path: /__properties         ┆ entity_path: /__properties          ┆                                   ┆                                    ┆ entity_path: /__properties/location                          │
│                                  ┆ kind: data                         ┆ kind: data                          ┆                                   ┆                                    ┆ kind: data                                                   │
╞══════════════════════════════════╪════════════════════════════════════╪═════════════════════════════════════╪═══════════════════════════════════╪════════════════════════════════════╪══════════════════════════════════════════════════════════════╡
│ 4cc4df9667fd4c308c4e3511b5e0da98 ┆ [segment_0]                        ┆ [1769101329662761000]               ┆ [0]                               ┆ [false]                            ┆ [[46.5, 6.5]]                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 473ec142a9464d51a1ee51cac71304ac ┆ [segment_1]                        ┆ [1769101329954056000]               ┆ [1]                               ┆ [true]                             ┆ [[46.5, 6.5]]                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 444dc0d31567473ab6dd5e48c53423e5 ┆ [segment_2]                        ┆ [1769101329955512000]               ┆ [2]                               ┆ [false]                            ┆ [[46.5, 6.5]]                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ c66ec19194ca48648799d1e9d60c6fe6 ┆ [segment_3]                        ┆ [1769101329956199000]               ┆ [3]                               ┆ [true]                             ┆ [[46.5, 6.5]]                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 2ef8804c3979415a8f8d35dc2b2adfa4 ┆ [segment_4]                        ┆ [1769101329957042000]               ┆ [4]                               ┆ [false]                            ┆ [[46.5, 6.5]]                                                │
└──────────────────────────────────┴────────────────────────────────────┴─────────────────────────────────────┴───────────────────────────────────┴────────────────────────────────────┴──────────────────────────────────────────────────────────────┘
```

The segment table contains one row per recording, with each property appearing as a column.
The column metadata exposes the fact that properties are stored under the reserved `/__properties` entity path.
For simplicity, the column names are however prefixed with `property:` instead of the full entity path.

Since the segment table is a [DataFusion](https://datafusion.apache.org/) DataFrame, you can use standard DataFrame operations for further processing and/or data conversion.
For example, this is how the segment table can be filtered based on the values of a custom property:

snippet: concepts/query-and-transform/segment_properties[filter]

Output:

```
┌──────────────────────────────────┬────────────────────────────────────┬─────────────────────────────────────┬───────────────────────────────────┬────────────────────────────────────┬──────────────────────────────────────────────────────────────┐
│ rerun_segment_id                 ┆ property:RecordingInfo:name        ┆ property:RecordingInfo:start_time   ┆ property:info:index               ┆ property:info:is_odd               ┆ property:location:GeoPoints:positions                        │
│ ---                              ┆ ---                                ┆ ---                                 ┆ ---                               ┆ ---                                ┆ ---                                                          │
│ type: Utf8                       ┆ type: nullable List[nullable Utf8] ┆ type: nullable List[nullable i64]   ┆ type: nullable List[nullable i64] ┆ type: nullable List[nullable bool] ┆ type: nullable List[nullable FixedSizeList[nullable f64; 2]] │
│                                  ┆ archetype: RecordingInfo           ┆ archetype: RecordingInfo            ┆ component: index                  ┆ component: is_odd                  ┆ archetype: GeoPoints                                         │
│                                  ┆ component: RecordingInfo:name      ┆ component: RecordingInfo:start_time ┆ entity_path: /__properties/info   ┆ entity_path: /__properties/info    ┆ component: GeoPoints:positions                               │
│                                  ┆ component_type: Name               ┆ component_type: Timestamp           ┆ kind: data                        ┆ kind: data                         ┆ component_type: LatLon                                       │
│                                  ┆ entity_path: /__properties         ┆ entity_path: /__properties          ┆                                   ┆                                    ┆ entity_path: /__properties/location                          │
│                                  ┆ kind: data                         ┆ kind: data                          ┆                                   ┆                                    ┆ kind: data                                                   │
╞══════════════════════════════════╪════════════════════════════════════╪═════════════════════════════════════╪═══════════════════════════════════╪════════════════════════════════════╪══════════════════════════════════════════════════════════════╡
│ 473ec142a9464d51a1ee51cac71304ac ┆ [segment_1]                        ┆ [1769101329954056000]               ┆ [1]                               ┆ [true]                             ┆ [[46.5, 6.5]]                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ c66ec19194ca48648799d1e9d60c6fe6 ┆ [segment_3]                        ┆ [1769101329956199000]               ┆ [3]                               ┆ [true]                             ┆ [[46.5, 6.5]]                                                │
└──────────────────────────────────┴────────────────────────────────────┴─────────────────────────────────────┴───────────────────────────────────┴────────────────────────────────────┴──────────────────────────────────────────────────────────────┘
```


## FAQ


### How are properties stored in the recording?

Properties are stored under the reserved `/__properties` entity path and are logged as [static data](../logging-and-ingestion/static.md), meaning they have no timeline association.
Logging directly under this entity path is not recommended.
Use the [`rr.send_property()`](https://ref.rerun.io/docs/python/stable/common/property_functions/#rerun.send_property?speculative-link) or [`RecordingStream.send_property()`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.RecordingStream.send_property?speculative-link) API instead.

### How are property columns named?

Property column names follow this general pattern:

```
property:$property_name:$Archetype:$field
```

where `$property_name` is the name provided to `send_property()`, and `$Archetype:$field` is derived from the property data.

For example, a `GeoPoints` archetype logged under the entity `location` appears as `property:location:GeoPoints:positions`.

For built-in properties, the `$property_name` part is omitted, e.g., `property:RecordingInfo:name`.

The `rr.AnyValues` helper logs data without a defined archetype.
As a result, the corresponding columns do not have the `$Archetype` part, e.g., `property:info:index`.

<!-- TODO(ab): should we be talking about DynamicArchetype here too? -->

### Are properties visible in dataframe queries?

Yes.

[Dataframe queries](dataframe-queries.md) can access properties by explicitly including the `/__properties/**` entity path filter, which is excluded by default.
When queried this way, the property column names follow the same rules described above.

snippet: concepts/query-and-transform/segment_properties[query]

Output:

```
┌──────────────────────────────────┬────────────────────────────────────┬───────────────────────────────────┐
│ rerun_segment_id                 ┆ property:RecordingInfo:name        ┆ property:info:index               │
│ ---                              ┆ ---                                ┆ ---                               │
│ type: Utf8                       ┆ type: nullable List[nullable Utf8] ┆ type: nullable List[nullable i64] │
│                                  ┆ archetype: RecordingInfo           ┆ component: index                  │
│                                  ┆ component: RecordingInfo:name      ┆ entity_path: /__properties/info   │
│                                  ┆ component_type: Name               ┆ is_static: true                   │
│                                  ┆ entity_path: /__properties         ┆ kind: data                        │
│                                  ┆ is_static: true                    ┆                                   │
│                                  ┆ kind: data                         ┆                                   │
╞══════════════════════════════════╪════════════════════════════════════╪═══════════════════════════════════╡
│ 8e59232215294cb39bc35dad5605bdce ┆ [segment_0]                        ┆ [0]                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 2067b4357dd744648ee0462f39c4de14 ┆ [segment_1]                        ┆ [1]                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 2cf0ff4e3a8c4cf3a0e00e88050cdb01 ┆ [segment_2]                        ┆ [2]                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 780ac12de8694dc7a9a916ccbb8a7218 ┆ [segment_3]                        ┆ [3]                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 99bc7f4dc0c5469c9d154a1cb01e002c ┆ [segment_4]                        ┆ [4]                               │
└──────────────────────────────────┴────────────────────────────────────┴───────────────────────────────────┘
```


### Can the built-in properties be omitted from recordings?

Yes.
Both [`rr.init()`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.init) and [`RecordingStream()`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.RecordingStream) accept a `send_properties` parameter (default: `True`).
Set it to `False` to prevent the built-in `RecordingInfo` properties from being automatically sent when the recording is created.
