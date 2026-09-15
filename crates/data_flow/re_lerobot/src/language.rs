//! Fan-out of `LeRobot` language annotation columns into per-entity text tracks.

use std::collections::BTreeMap;

use arrow::array::{ArrayRef, Float64Array, ListArray, StringArray, StructArray};
use arrow::compute::cast;
use arrow::datatypes::DataType;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{ArrowArray as _, TimeInt};

use crate::emits::LANGUAGE_PERSISTENT_COLUMN;

/// Resolved text rows for one entity: `(time, text)` pairs.
pub type TextRows = Vec<(TimeInt, String)>;

/// Read a timestamp-like column as `f64`.
///
/// `LeRobot` types timestamps as `float32` in places and `float64` in others, and datasets
/// vary; casting to a common `f64` lets either load. Returns `None` if the column is
/// absent or not castable.
pub fn timestamps_as_f64(column: Option<&ArrayRef>) -> Option<Float64Array> {
    let casted = cast(column?, &DataType::Float64).ok()?;
    casted.downcast_array_ref::<Float64Array>().cloned()
}

/// Read the string at index `i`, returning `None` if the array is absent or the value is null.
fn value_at(array: Option<&StringArray>, i: usize) -> Option<&str> {
    let a = array?;
    a.is_valid(i).then(|| a.value(i))
}

/// Render the elements of a single `tool_calls` list to text, one element per line.
fn tool_calls_to_text(elements: &dyn arrow::array::Array) -> Option<String> {
    use arrow::util::display::{ArrayFormatter, FormatOptions};
    let formatter = ArrayFormatter::try_new(elements, &FormatOptions::default()).ok()?;
    let joined = (0..elements.len())
        .map(|i| formatter.value(i).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    (!joined.is_empty()).then_some(joined)
}

/// Resolve a language column into its per-entity text tracks, one `(entity, rows)` pair
/// per fanned-out sub-entity. The rows are owned, so the result borrows nothing.
///
/// The rows are placed in frame space; `frame_for_timestamp` maps a persistent
/// annotation's emission timestamp (in seconds) to its frame position.
pub fn resolve_language_tracks(
    column: &str,
    list: &ListArray,
    frame_for_timestamp: &dyn Fn(f64) -> i64,
) -> Vec<(String, TextRows)> {
    let Some(representative_idx) = (0..list.len()).find(|&i| list.value_length(i) > 0) else {
        return vec![];
    };

    let representative_rows = list.value(representative_idx);
    let Some(representative_rows) = representative_rows.downcast_array_ref::<StructArray>() else {
        re_log::warn_once!(
            "LeRobot language feature `{column}` is not a list of annotation rows; skipping"
        );
        return vec![];
    };

    let is_persistent = column == LANGUAGE_PERSISTENT_COLUMN;

    // Collect in i64 frame space first to match collect_language_rows, then convert to TimeInt.
    // Keyed by entity path in an ordered map, so the tracks — and with them the chunk output
    // order — are deterministic.
    let mut by_entity: BTreeMap<String, Vec<(i64, String)>> = BTreeMap::new();

    if is_persistent {
        let row_timestamps = timestamps_as_f64(representative_rows.column_by_name("timestamp"));

        collect_language_rows(column, representative_rows, &mut by_entity, |i| {
            row_timestamps
                .as_ref()
                .filter(|ts| ts.is_valid(i))
                .map_or(0, |ts| frame_for_timestamp(ts.value(i)))
        });
    } else {
        for frame in 0..list.len() {
            if list.value_length(frame) == 0 {
                continue;
            }
            let rows = list.value(frame);
            let Some(rows) = rows.downcast_array_ref::<StructArray>() else {
                continue;
            };
            let frame = i64::try_from(frame).unwrap_or(0);
            collect_language_rows(column, rows, &mut by_entity, |_| frame);
        }
    }

    let mut tracks = Vec::with_capacity(by_entity.len());
    for (entity_path, mut annotations) in by_entity {
        annotations.sort_by_key(|(frame, _)| *frame);
        let rows = annotations
            .into_iter()
            .map(|(frame, content)| (TimeInt::new_temporal(frame), content))
            .collect();
        tracks.push((entity_path, rows));
    }

    tracks
}

/// Collect language annotation rows into per-entity `(frame, content)` pairs.
///
/// Each row is routed to an entity path of `{feature}/{style}[/{role}][/{camera}]` and placed on
/// the frame returned by `frame_of_row` (its emission frame for persistent rows, or the frame it
/// occupies for events). Rows with neither `content` nor `tool_calls` produce nothing.
fn collect_language_rows(
    column: &str,
    rows: &StructArray,
    by_entity: &mut BTreeMap<String, Vec<(i64, String)>>,
    frame_of_row: impl Fn(usize) -> i64,
) {
    let styles = rows
        .column_by_name("style")
        .and_then(|c| c.downcast_array_ref::<StringArray>());
    let contents = rows
        .column_by_name("content")
        .and_then(|c| c.downcast_array_ref::<StringArray>());
    let roles = rows
        .column_by_name("role")
        .and_then(|c| c.downcast_array_ref::<StringArray>());
    let cameras = rows
        .column_by_name("camera")
        .and_then(|c| c.downcast_array_ref::<StringArray>());
    // `tool_calls` is a `list<struct>` of OpenAI-style function calls (real on-disk shape:
    // `list<struct<type, function<name, arguments<…>>>>`); we render each element generically.
    let tool_calls = rows
        .column_by_name("tool_calls")
        .and_then(|c| c.downcast_array_ref::<ListArray>());

    for i in 0..rows.len() {
        // Key by the present distinguishing fields, in resolver order (style, role, camera).
        // `style` and `camera` are nullable (e.g. `say` speech rows have no style), so we simply
        // omit any absent segment rather than inventing a placeholder.
        let mut entity_path = column.to_owned();
        for segment in [
            value_at(styles, i),
            value_at(roles, i),
            value_at(cameras, i),
        ]
        .into_iter()
        .flatten()
        {
            entity_path.push('/');
            entity_path.push_str(segment);
        }

        let frame = frame_of_row(i);

        // Tool calls (if any) go on a `…/tool_calls` sub-entity.
        if let Some(tool_calls) = tool_calls
            && tool_calls.is_valid(i)
            && tool_calls.value_length(i) > 0
            && let Some(text) = tool_calls_to_text(tool_calls.value(i).as_ref())
        {
            by_entity
                .entry(format!("{entity_path}/tool_calls"))
                .or_default()
                .push((frame, text));
        }

        if let Some(content) = value_at(contents, i) {
            by_entity
                .entry(entity_path)
                .or_default()
                .push((frame, content.to_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use arrow::array::Float32Array;
    use arrow::buffer::OffsetBuffer;
    use arrow::datatypes::{Field, Fields};
    use re_chunk::{Chunk, EntityPath, Timeline};

    use crate::LeRobotError;

    /// A single `LeRobot` language annotation row, used to build synthetic test data.
    struct Row {
        style: Option<&'static str>,
        content: Option<&'static str>,
        role: Option<&'static str>,
        camera: Option<&'static str>,
        timestamp: Option<f64>,
        tool_calls: Option<Vec<&'static str>>,
    }

    /// Build a `list<struct>` language column from per-frame rows.
    fn language_column(frames: &[Vec<Row>]) -> ListArray {
        let all: Vec<&Row> = frames.iter().flatten().collect();

        // Build `tool_calls` in its real on-disk shape — an OpenAI-style function-call struct,
        // `list<struct<function<name, arguments<text>>, type>>` — rather than flattening it to a
        // list of strings, so the test drives the importer's generic struct rendering. Each call is
        // a `say` function (the catalog's canonical/default tool) whose `arguments.text` is the
        // spoken utterance. See https://huggingface.co/docs/lerobot/en/tools
        let say_texts: Vec<&str> = all
            .iter()
            .flat_map(|r| r.tool_calls.clone().unwrap_or_default())
            .collect();
        let arguments_fields = Fields::from(vec![Field::new("text", DataType::Utf8, true)]);
        let function_fields = Fields::from(vec![
            Field::new(
                "arguments",
                DataType::Struct(arguments_fields.clone()),
                true,
            ),
            Field::new("name", DataType::Utf8, true),
        ]);
        let tool_call_fields = Fields::from(vec![
            Field::new("function", DataType::Struct(function_fields.clone()), true),
            Field::new("type", DataType::Utf8, true),
        ]);
        let arguments = StructArray::new(
            arguments_fields,
            vec![Arc::new(
                say_texts.iter().map(|t| Some(*t)).collect::<StringArray>(),
            )],
            None,
        );
        let function = StructArray::new(
            function_fields,
            vec![
                Arc::new(arguments),
                Arc::new(
                    say_texts
                        .iter()
                        .map(|_| Some("say"))
                        .collect::<StringArray>(),
                ),
            ],
            None,
        );
        let tool_call_values = StructArray::new(
            tool_call_fields.clone(),
            vec![
                Arc::new(function),
                Arc::new(
                    say_texts
                        .iter()
                        .map(|_| Some("function"))
                        .collect::<StringArray>(),
                ),
            ],
            None,
        );
        let tool_calls_item = Field::new("item", DataType::Struct(tool_call_fields), true);
        let tool_calls = ListArray::new(
            Arc::new(tool_calls_item.clone()),
            OffsetBuffer::from_lengths(
                all.iter()
                    .map(|r| r.tool_calls.as_ref().map_or(0, Vec::len)),
            ),
            Arc::new(tool_call_values),
            None,
        );

        // `timestamp` is `float32` in the source schema (some datasets store `float64`) — use
        // `float32` here so the importer's float32→f64 handling is exercised.
        let struct_fields = Fields::from(vec![
            Field::new("style", DataType::Utf8, true),
            Field::new("content", DataType::Utf8, true),
            Field::new("role", DataType::Utf8, true),
            Field::new("camera", DataType::Utf8, true),
            Field::new("timestamp", DataType::Float32, true),
            Field::new(
                "tool_calls",
                DataType::List(Arc::new(tool_calls_item)),
                true,
            ),
        ]);
        let values = StructArray::new(
            struct_fields.clone(),
            vec![
                Arc::new(all.iter().map(|r| r.style).collect::<StringArray>()),
                Arc::new(all.iter().map(|r| r.content).collect::<StringArray>()),
                Arc::new(all.iter().map(|r| r.role).collect::<StringArray>()),
                Arc::new(all.iter().map(|r| r.camera).collect::<StringArray>()),
                Arc::new(
                    all.iter()
                        .map(|r| r.timestamp.map(|t| t as f32))
                        .collect::<Float32Array>(),
                ),
                Arc::new(tool_calls),
            ],
            None,
        );
        let offsets = OffsetBuffer::from_lengths(frames.iter().map(Vec::len));
        let item = Field::new("item", DataType::Struct(struct_fields), true);
        ListArray::new(Arc::new(item), offsets, Arc::new(values), None)
    }

    /// Resolve a language column and emit its text chunks — the two halves the old
    /// `log_episode_language` performed in one step.
    fn language_chunks(
        column: &str,
        timeline: &Timeline,
        list: &ListArray,
    ) -> impl Iterator<Item = Result<Chunk, LeRobotError>> + use<> {
        let timeline = *timeline;
        // The tests place timestamps on a 1 fps grid, so a frame is the rounded timestamp.
        #[expect(clippy::cast_possible_truncation)]
        let frame_for_timestamp = |ts: f64| ts.round() as i64;
        resolve_language_tracks(column, list, &frame_for_timestamp)
            .into_iter()
            .map(move |(entity, rows)| {
                crate::convert::build_text_chunk(
                    &EntityPath::parse_forgiving(&entity),
                    &rows,
                    &timeline,
                )
            })
    }

    fn entity_paths(chunks: &[Chunk]) -> Vec<String> {
        let mut paths: Vec<_> = chunks.iter().map(|c| c.entity_path().to_string()).collect();
        paths.sort();
        paths
    }

    fn chunk<'a>(chunks: &'a [Chunk], entity_path: &str) -> &'a Chunk {
        chunks
            .iter()
            .find(|c| c.entity_path().to_string() == entity_path)
            .unwrap_or_else(|| panic!("missing chunk for `{entity_path}`"))
    }

    /// Render every logged component value in a chunk to text (for asserting on document contents).
    fn rendered_text(chunk: &Chunk) -> String {
        use arrow::util::display::{ArrayFormatter, FormatOptions};
        chunk
            .components()
            .values()
            .flat_map(|col| {
                let formatter =
                    ArrayFormatter::try_new(&col.list_array, &FormatOptions::default()).unwrap();
                (0..col.list_array.len())
                    .map(|i| formatter.value(i).to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Persistent rows carry a timestamp, are broadcast to every frame, and are placed on the frame
    /// matching their emission timestamp. View-dependent (`camera`) and role-paired rows must not
    /// collapse onto the same entity.
    #[test]
    fn language_persistent_broadcast_and_keying() {
        // Three frames, each broadcasting the same three persistent rows.
        let rows = || {
            vec![
                Row {
                    style: Some("subtask"),
                    content: Some("pick"),
                    role: Some("assistant"),
                    camera: None,
                    timestamp: Some(0.0),
                    tool_calls: None,
                },
                Row {
                    style: Some("subtask"),
                    content: Some("place"),
                    role: Some("assistant"),
                    camera: None,
                    timestamp: Some(2.0),
                    tool_calls: None,
                },
                Row {
                    style: Some("vqa"),
                    content: Some("what is it?"),
                    role: Some("user"),
                    camera: Some("observation.images.top"),
                    timestamp: Some(1.0),
                    tool_calls: None,
                },
            ]
        };
        let frames = vec![rows(), rows(), rows()];

        let timeline = Timeline::new_sequence("frame_index");
        let chunks = language_chunks("language_persistent", &timeline, &language_column(&frames))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            entity_paths(&chunks),
            vec![
                "/language_persistent/subtask/assistant".to_owned(),
                "/language_persistent/vqa/user/observation.images.top".to_owned(),
            ]
        );
        // Two subtasks placed at frames 0 (ts 0.0) and 2 (ts 2.0).
        assert_eq!(
            chunk(&chunks, "/language_persistent/subtask/assistant").num_rows(),
            2
        );
        // One vqa query at frame 1 (ts 1.0), kept separate by role + camera.
        assert_eq!(
            chunk(
                &chunks,
                "/language_persistent/vqa/user/observation.images.top"
            )
            .num_rows(),
            1
        );
    }

    /// Event rows omit the timestamp and exist only on the exact frame they were emitted on, so each
    /// must be placed on the frame it occupies rather than broadcast.
    #[test]
    fn language_events_placed_per_frame() {
        let frames = vec![
            vec![],
            vec![Row {
                style: Some("interjection"),
                content: Some("stop!"),
                role: Some("assistant"),
                camera: None,
                timestamp: None,
                tool_calls: None,
            }],
            vec![],
        ];

        let timeline = Timeline::new_sequence("frame_index");
        let chunks = language_chunks("language_events", &timeline, &language_column(&frames))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            entity_paths(&chunks),
            vec!["/language_events/interjection/assistant".to_owned()]
        );
        assert_eq!(
            chunk(&chunks, "/language_events/interjection/assistant").num_rows(),
            1
        );
    }

    /// A pure tool-call event row (null `content`, non-empty `tool_calls`) is not dropped: its calls
    /// are captured as text on a `…/tool_calls` sub-entity.
    #[test]
    fn language_tool_calls_captured_on_sub_entity() {
        let frames = vec![
            vec![],
            vec![Row {
                style: None, // speech rows carry no style
                content: None,
                role: Some("assistant"),
                camera: None,
                timestamp: None,
                tool_calls: Some(vec!["hello there"]), // the `say` text
            }],
        ];

        let timeline = Timeline::new_sequence("frame_index");
        let chunks = language_chunks("language_events", &timeline, &language_column(&frames))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // No content entity (content was null), only the tool-call sub-entity. Style is null (speech
        // rows carry none), so the path omits the style segment rather than inventing a placeholder.
        assert_eq!(
            entity_paths(&chunks),
            vec!["/language_events/assistant/tool_calls".to_owned()]
        );
        let tool_call_chunk = chunk(&chunks, "/language_events/assistant/tool_calls");
        assert_eq!(tool_call_chunk.num_rows(), 1);
        // The generic struct render keeps the `say` text, even if it's buried in the struct dump.
        assert!(
            rendered_text(tool_call_chunk).contains("hello there"),
            "rendered tool call should retain the say text, got: {}",
            rendered_text(tool_call_chunk)
        );
    }

    /// An all-null / empty language column (e.g. an unused `language_events`) yields no chunks.
    #[test]
    fn language_empty_column_is_skipped() {
        let frames: Vec<Vec<Row>> = vec![vec![], vec![], vec![]];
        let timeline = Timeline::new_sequence("frame_index");
        let chunks = language_chunks("language_events", &timeline, &language_column(&frames))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(chunks.is_empty());
    }

    /// The `style`/`role`/`camera` path segments are dataset-authored strings we don't control, so
    /// they can be empty or contain characters (`/`, spaces, `!`) that are meaningful in an entity
    /// path. `EntityPath::parse_forgiving` must absorb these without panicking, and degrade sanely:
    /// empty segments collapse (dropped duplicate slash), an embedded `/` just nests deeper, and
    /// other characters get escaped.
    #[test]
    fn language_edge_case_path_segments_are_forgiving() {
        let frames = vec![
            vec![],
            vec![
                // Empty style: the `//` it would produce collapses to a single separator.
                Row {
                    style: Some(""),
                    content: Some("empty style"),
                    role: Some("assistant"),
                    camera: None,
                    timestamp: None,
                    tool_calls: None,
                },
                // A `/` inside a segment splits into extra path parts rather than erroring.
                Row {
                    style: Some("vqa"),
                    content: Some("slashy camera"),
                    role: Some("user"),
                    camera: Some("observation/images/top"),
                    timestamp: None,
                    tool_calls: None,
                },
                // Spaces and `!` are escaped, not rejected.
                Row {
                    style: Some("pick up!"),
                    content: Some("special chars"),
                    role: None,
                    camera: None,
                    timestamp: None,
                    tool_calls: None,
                },
            ],
        ];

        let timeline = Timeline::new_sequence("frame_index");
        // Must not panic on any of the awkward segments.
        let chunks = language_chunks("language_events", &timeline, &language_column(&frames))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let paths = entity_paths(&chunks);
        // Empty style drops the duplicate slash; the `/` in the camera nests deeper; the special
        // characters survive (escaped) rather than breaking the path. Three distinct entities.
        assert_eq!(
            paths,
            vec![
                "/language_events/assistant".to_owned(),
                "/language_events/pick\\ up\\!".to_owned(),
                "/language_events/vqa/user/observation/images/top".to_owned(),
            ]
        );
    }

    /// Manual verification against a real `LeRobot` v0.6.0 dataset parquet (e.g.
    /// `pepijn223/human_new_35_annotated`). Ignored by default; run with:
    ///   `LEROBOT_LANG_PARQUET=/path/to/file-000.parquet cargo test -p re_lerobot --all-features
    ///   verify_real_language_parquet -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a local LeRobot v0.6.0 parquet via LEROBOT_LANG_PARQUET"]
    fn verify_real_language_parquet() {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let path = std::env::var("LEROBOT_LANG_PARQUET").expect("set LEROBOT_LANG_PARQUET");
        let file = std::fs::File::open(&path).unwrap();
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .unwrap()
            .build()
            .unwrap();
        let batches: Vec<arrow::array::RecordBatch> = reader.map(Result::unwrap).collect();
        let batch = arrow::compute::concat_batches(&batches[0].schema(), &batches).unwrap();

        let timeline = Timeline::new_sequence("frame_index");
        for feature in ["language_persistent", "language_events"] {
            let Some(list) = batch
                .column_by_name(feature)
                .and_then(|c| c.downcast_array_ref::<ListArray>())
            else {
                println!("\n===== {feature}: not present =====");
                continue;
            };
            let chunks = language_chunks(feature, &timeline, list)
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            println!("\n===== {feature}: {} entities =====", chunks.len());
            for c in &chunks {
                let preview: String = rendered_text(c)
                    .replace('\n', " ")
                    .chars()
                    .take(160)
                    .collect();
                println!(
                    "  {} ({} rows)\n      {preview}",
                    c.entity_path(),
                    c.num_rows()
                );
            }
        }
    }
}
