//! Lenses shaping the raw parquet column chunks into Rerun archetypes.
//!
//! Each lens is keyed by its input component identifier, which `re_parquet` sets to the
//! raw parquet column name, so routing happens on column names known at planning time.

use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, FixedSizeListArray, Int64Array, ListArray, StringArray};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{ComponentIdentifier, EntityPath};
use re_lenses::{Lens, LensBuilderError, Lenses, OutputMode, Selector, op};
use re_lenses_core::combinators::{Error, try_downcast};
use re_sdk_types::archetypes::{EncodedDepthImage, EncodedImage, Scalars, TextDocument};

use crate::dataset::Tasks;
use crate::emits::{TabularEmit, TabularEmitKind};

/// A `task_index`-style lookup table with the column's raw `Int64` key type.
type LabelTable = Arc<ahash::HashMap<i64, String>>;

/// Entries whose index does not fit in `i64` are dropped from the table.
fn label_table<'a>(entries: impl Iterator<Item = (usize, &'a String)>) -> LabelTable {
    Arc::new(
        entries
            .filter_map(|(index, label)| {
                i64::try_from(index)
                    .ok()
                    .map(|index| (index, label.clone()))
            })
            .collect(),
    )
}

/// Build the lens collection for one episode's lens-shaped emits.
pub fn build_lenses(emits: &[TabularEmit], tasks: &Tasks) -> Result<Lenses, LensBuilderError> {
    let task_labels = label_table(tasks.tasks.iter().map(|(index, label)| (index.0, label)));
    let subtask_labels = label_table(tasks.subtasks.iter().map(|(index, label)| (index.0, label)));

    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);
    for emit in emits {
        // A column name `re_parquet` cannot use as a component identifier never reaches
        // a chunk, so there is nothing for a lens to match either.
        let Ok(column) = ComponentIdentifier::try_new(&emit.column) else {
            re_log::warn!(
                "LeRobot feature `{}` has no valid component identifier; its column is not shaped",
                emit.column
            );
            continue;
        };
        let lens = match &emit.kind {
            TabularEmitKind::Scalars { vector, .. } => scalars_lens(column, *vector)?,
            TabularEmitKind::Image { depth: false } => encoded_image_lens(column)?,
            TabularEmitKind::Image { depth: true } => depth_image_lens(column)?,
            TabularEmitKind::TaskLabels => {
                index_labels_lens(column, emit.entity.clone(), task_labels.clone())?
            }
            TabularEmitKind::SubtaskLabels => {
                index_labels_lens(column, emit.entity.clone(), subtask_labels.clone())?
            }
            TabularEmitKind::Text => text_lens(column)?,
        };
        lenses = lenses.add_lens(lens);
    }
    Ok(lenses)
}

/// Shape a numeric feature column into the `Scalars` archetype: a vector feature keeps
/// all of each row's elements, a single-scalar feature is unwrapped from its one-element
/// lists to one value per row.
/// TODO(RR-5278): Potentially lossy transformations with `OutputMode::ForwardUnmatched`
fn scalars_lens(column: ComponentIdentifier, vector: bool) -> Result<Lens, LensBuilderError> {
    let selector = if vector {
        Selector::parse(".[]")?.pipe(scalars_as_f64())
    } else {
        Selector::parse(".")?
            .pipe(unwrap_single_element())
            .pipe(scalars_as_f64())
    };
    Lens::derive(column)
        .to_component(Scalars::descriptor_scalars(), selector)
        .build()
}

/// Cast a numeric column to `Float64`, the `Scalar` component's canonical type, so every
/// dataset registers scalars under one schema regardless of its on-disk precision.
fn scalars_as_f64() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source| {
        Ok(Some(arrow::compute::cast(
            source,
            &arrow::datatypes::DataType::Float64,
        )?))
    }
}

/// Rewrap an encoded-image column (`struct<bytes: binary>`) as the `EncodedImage`
/// archetype.
fn encoded_image_lens(column: ComponentIdentifier) -> Result<Lens, LensBuilderError> {
    Lens::derive(column)
        .to_component(
            EncodedImage::descriptor_blob(),
            Selector::parse(".bytes")?.pipe(op::binary_to_list_uint8()),
        )
        .build()
}

/// Rewrap a 1-channel encoded-image column (`struct<bytes: binary>`) as the
/// `EncodedDepthImage` archetype.
///
/// No media type is emitted: the viewer sniffs it from the blob's magic bytes, so the
/// lens stays byte-for-byte lossless and never decodes.
fn depth_image_lens(column: ComponentIdentifier) -> Result<Lens, LensBuilderError> {
    Lens::derive(column)
        .to_component(
            EncodedDepthImage::descriptor_blob(),
            Selector::parse(".bytes")?.pipe(op::binary_to_list_uint8()),
        )
        .build()
}

/// Shape a string feature column into the `TextDocument` archetype: each row's string is
/// unwrapped from its one-element list and cast to the `Text` component's canonical
/// `Utf8` type, whatever string encoding the parquet file stored.
fn text_lens(column: ComponentIdentifier) -> Result<Lens, LensBuilderError> {
    Lens::derive(column)
        .to_component(
            TextDocument::descriptor_text(),
            Selector::parse(".")?
                .pipe(unwrap_single_element())
                .pipe(strings_as_utf8()),
        )
        .build()
}

/// Cast a string column to `Utf8`, the `Text` component's canonical type.
fn strings_as_utf8() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source| {
        Ok(Some(arrow::compute::cast(
            source,
            &arrow::datatypes::DataType::Utf8,
        )?))
    }
}

/// Join an index column (`task_index`/`subtask_index`) against its label table, emitting
/// the labels as a `TextDocument` at `entity`.
///
/// The derived chunk inherits the input chunk's time columns, so the labels land on the
/// episode timeline. An index without a label warns once and becomes a null row (no
/// value logged).
fn index_labels_lens(
    column: ComponentIdentifier,
    entity: EntityPath,
    labels: LabelTable,
) -> Result<Lens, LensBuilderError> {
    Lens::derive(column)
        .output_entity(entity)
        .to_component(
            TextDocument::descriptor_text(),
            Selector::parse(".")?.pipe(lookup_table(labels, column)),
        )
        .build()
}

/// Map each `Int64` index through the table; missing entries become nulls.
fn lookup_table(
    table: LabelTable,
    column: ComponentIdentifier,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source| {
        let indices = try_downcast::<Int64Array>(source, "task index column")?;
        let labels: StringArray = indices
            .iter()
            .map(|index| {
                let index = index?;
                let label = table.get(&index);
                if label.is_none() {
                    re_log::warn_once!(
                        "A frame references `{column}` {index}, which is not defined in its label table"
                    );
                }
                label.map(String::as_str)
            })
            .collect();
        Ok(Some(Arc::new(labels) as ArrayRef))
    }
}

/// Unwrap a single-scalar feature stored as one-element lists back to a plain column;
/// any other shape passes through unchanged.
///
/// The values buffer is only taken when it is row-aligned (no slicing offset), so a
/// misaligned array degrades to pass-through rather than misattributing values to rows.
fn unwrap_single_element() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source| {
        if let Some(fixed) = source.downcast_array_ref::<FixedSizeListArray>()
            && fixed.value_length() == 1
            && fixed.values().len() == fixed.len()
        {
            return Ok(Some(fixed.values().clone()));
        }
        if let Some(list) = source.downcast_array_ref::<ListArray>()
            && is_row_aligned_unit_list(list)
        {
            return Ok(Some(list.values().clone()));
        }
        Ok(Some(source.clone()))
    }
}

/// Whether every row is a one-element list over an unsliced child, so the child array is
/// row-aligned with the list: `child[i]` is row `i`'s value.
///
/// The unit-length walk rules out ragged and empty rows; combined with the length check
/// it also rules out a slicing offset, since the last offset can be at most the child
/// length, which pins the first offset to zero.
pub fn is_row_aligned_unit_list(list: &ListArray) -> bool {
    list.values().len() == list.len()
        && list.offsets().windows(2).all(|pair| pair[1] - pair[0] == 1)
}

/// Pins what each feature lens does to a raw `re_parquet` column chunk: shape changes
/// must change a test here.
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{BinaryArray, Float32Array, StructArray};
    use arrow::datatypes::{DataType, Field};
    use re_chunk::{Chunk, ChunkId, EntityPath, TimeColumn, Timeline};
    use re_sdk_types::ComponentDescriptor;

    use super::*;

    /// The alignment proof holds only for unit-length rows over an unsliced child:
    /// ragged offsets and sliced arrays must both be rejected.
    #[test]
    fn row_alignment_rejects_ragged_and_sliced_lists() {
        let field = Arc::new(Field::new("item", DataType::Int64, true));
        let values: ArrayRef = Arc::new(Int64Array::from(vec![10_i64, 20, 30]));

        let unit = ListArray::try_new(
            field.clone(),
            arrow::buffer::OffsetBuffer::from_lengths([1, 1, 1]),
            values.clone(),
            None,
        )
        .unwrap();
        assert!(is_row_aligned_unit_list(&unit));

        // Sliced: child length no longer matches, and taking the child whole would
        // shift every row.
        let sliced = unit.slice(1, 2);
        assert!(!is_row_aligned_unit_list(&sliced));

        // Ragged offsets `[0, 0, 3]`: row 0 is empty and row 1 holds all three values,
        // so the child is not row-aligned even though it starts at offset 0.
        let ragged = ListArray::try_new(
            field,
            arrow::buffer::OffsetBuffer::from_lengths([0, 3]),
            values,
            None,
        )
        .unwrap();
        assert!(!is_row_aligned_unit_list(&ragged));
    }
    use crate::emits::TabularEmit;

    /// One chunk shaped like `re_parquet`'s output: a single data column named `column`,
    /// wrapped one element per row, on a `frame_index` timeline.
    fn column_chunk(column: &str, values: ArrayRef) -> Chunk {
        let field = Arc::new(Field::new("item", values.data_type().clone(), true));
        let offsets =
            arrow::buffer::OffsetBuffer::from_lengths(std::iter::repeat_n(1, values.len()));
        let list = ListArray::try_new(field, offsets, values, None).unwrap();

        let timeline = Timeline::new_sequence("frame_index");
        let times = TimeColumn::new(
            None,
            timeline,
            (0..i64::try_from(list.len()).unwrap())
                .collect::<Vec<_>>()
                .into(),
        );

        let descriptor =
            ComponentDescriptor::partial(ComponentIdentifier::try_new(column).unwrap());
        let components: re_chunk::ChunkComponents = std::iter::once((descriptor, list)).collect();
        Chunk::from_auto_row_ids(
            ChunkId::new(),
            EntityPath::from(format!("/{column}")),
            std::iter::once((*timeline.name(), times)).collect(),
            components,
        )
        .unwrap()
    }

    fn apply(lenses: &Lenses, chunk: &Chunk) -> Vec<Chunk> {
        lenses
            .apply(chunk, &re_lenses::default_runtime())
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn scalar_emit(column: &str, vector: bool) -> TabularEmit {
        TabularEmit {
            column: column.to_owned(),
            entity: EntityPath::from(format!("/{column}")),
            kind: TabularEmitKind::Scalars {
                names: Vec::new(),
                vector,
            },
        }
    }

    fn image_emit(column: &str, depth: bool) -> TabularEmit {
        TabularEmit {
            column: column.to_owned(),
            entity: EntityPath::from(format!("/{column}")),
            kind: TabularEmitKind::Image { depth },
        }
    }

    /// A vector feature flattens so each row carries its elements, cast to `Float64`,
    /// for both list layouts a parquet column can store.
    #[test]
    fn vector_scalars_flatten_and_cast_to_f64() {
        let element = Arc::new(Field::new("item", DataType::Float32, true));
        let flat = Arc::new(Float32Array::from(vec![1.0_f32, 2.0, 3.0, 4.0]));
        let fixed: ArrayRef = Arc::new(
            arrow::array::FixedSizeListArray::try_new(element.clone(), 2, flat.clone(), None)
                .unwrap(),
        );
        let listed: ArrayRef = Arc::new(
            ListArray::try_new(
                element,
                arrow::buffer::OffsetBuffer::from_lengths([2, 2]),
                flat,
                None,
            )
            .unwrap(),
        );

        let lenses = build_lenses(&[scalar_emit("action", true)], &Tasks::default()).unwrap();
        for values in [fixed, listed] {
            let out = apply(&lenses, &column_chunk("action", values));
            assert_eq!(out.len(), 1, "one chunk in, one chunk out");
            let chunk = &out[0];
            assert_eq!(chunk.entity_path(), &EntityPath::from("/action"));
            assert_eq!(chunk.components().len(), 1, "the raw column is consumed");

            let scalars = chunk
                .components()
                .get(Scalars::descriptor_scalars().component)
                .unwrap();
            assert_eq!(scalars.list_array.len(), 2);
            assert_eq!(scalars.list_array.value(0).len(), 2);
            assert_eq!(scalars.list_array.values().data_type(), &DataType::Float64);
            assert!(chunk.timelines().contains_key(&"frame_index".into()));
        }
    }

    /// A single-scalar feature keeps one value per row, whether stored plain or as
    /// one-element lists.
    #[test]
    fn single_scalars_pass_through() {
        let plain: ArrayRef = Arc::new(Float32Array::from(vec![1.0_f32, 2.0]));
        let element = Arc::new(Field::new("item", DataType::Float32, true));
        let wrapped: ArrayRef = Arc::new(
            arrow::array::FixedSizeListArray::try_new(
                element,
                1,
                Arc::new(Float32Array::from(vec![1.0_f32, 2.0])),
                None,
            )
            .unwrap(),
        );

        let lenses = build_lenses(&[scalar_emit("reward", false)], &Tasks::default()).unwrap();
        for values in [plain, wrapped] {
            let out = apply(&lenses, &column_chunk("reward", values));
            let scalars = out[0]
                .components()
                .get(Scalars::descriptor_scalars().component)
                .unwrap();
            assert_eq!(scalars.list_array.len(), 2);
            assert_eq!(scalars.list_array.value(0).len(), 1);
            assert_eq!(scalars.list_array.values().data_type(), &DataType::Float64);
        }
    }

    /// An rgb image column becomes an `EncodedImage` blob, and nothing else.
    #[test]
    fn encoded_images_carry_only_the_blob() {
        let png_header: &[u8] = b"\x89PNG\r\n\x1a\n0000";
        let bytes: ArrayRef = Arc::new(BinaryArray::from(vec![png_header, png_header]));
        let values: ArrayRef = Arc::new(StructArray::from(vec![(
            Arc::new(Field::new("bytes", DataType::Binary, true)),
            bytes,
        )]));

        let lenses =
            build_lenses(&[image_emit("observation.image", false)], &Tasks::default()).unwrap();
        let out = apply(&lenses, &column_chunk("observation.image", values));
        assert_eq!(out.len(), 1);
        let chunk = &out[0];
        assert_eq!(chunk.components().len(), 1, "no media type is emitted");

        let blob = chunk
            .components()
            .get(EncodedImage::descriptor_blob().component)
            .unwrap();
        assert_eq!(blob.list_array.len(), 2);
        assert_eq!(
            blob.list_array.value(0).len(),
            1,
            "one blob per row, holding the image bytes"
        );
    }

    /// A depth column becomes an `EncodedDepthImage` blob carrying the bytes untouched,
    /// and nothing else: decoding is the viewer's job.
    #[test]
    fn depth_images_carry_only_the_blob() {
        let raw: &[u8] = b"not an image";
        let bytes: ArrayRef = Arc::new(BinaryArray::from(vec![raw]));
        let values: ArrayRef = Arc::new(StructArray::from(vec![(
            Arc::new(Field::new("bytes", DataType::Binary, true)),
            bytes,
        )]));

        let lenses =
            build_lenses(&[image_emit("observation.depth", true)], &Tasks::default()).unwrap();
        let out = apply(&lenses, &column_chunk("observation.depth", values));
        let chunk = &out[0];
        assert_eq!(chunk.components().len(), 1, "no media type is emitted");

        let blob = chunk
            .components()
            .get(EncodedDepthImage::descriptor_blob().component)
            .unwrap();
        let row = blob.list_array.value(0);
        let row = row.downcast_array_ref::<ListArray>().unwrap();
        assert_eq!(
            row.value(0).len(),
            raw.len(),
            "the blob carries the raw bytes untouched"
        );
    }

    /// A null depth row is a frame without a depth image: it must stay null in the blob
    /// column, not become a fabricated empty blob.
    #[test]
    fn null_depth_rows_stay_null() {
        let raw: &[u8] = b"not an image";
        let bytes: ArrayRef = Arc::new(BinaryArray::from(vec![Some(raw), None]));
        let values: ArrayRef = Arc::new(StructArray::from(vec![(
            Arc::new(Field::new("bytes", DataType::Binary, true)),
            bytes,
        )]));

        let lenses =
            build_lenses(&[image_emit("observation.depth", true)], &Tasks::default()).unwrap();
        let out = apply(&lenses, &column_chunk("observation.depth", values));
        let chunk = &out[0];

        let blob = chunk
            .components()
            .get(EncodedDepthImage::descriptor_blob().component)
            .unwrap();
        assert_eq!(blob.list_array.len(), 2);
        let encoded = blob.list_array.value(0);
        assert!(encoded.is_valid(0), "the encoded row stays valid");
        let missing = blob.list_array.value(1);
        assert!(missing.is_null(0), "the null row stays null");
    }

    /// A `task_index` column joins against the label table on its own entity, with the
    /// time column carried over and unknown indices left as null rows.
    #[test]
    fn task_indices_join_to_text_labels() {
        use crate::dataset::TaskIndex;

        let values: ArrayRef = Arc::new(arrow::array::Int64Array::from(vec![0_i64, 7, 1]));
        let chunk = column_chunk("task_index", values);

        let emit = TabularEmit {
            column: "task_index".to_owned(),
            entity: EntityPath::from("/task"),
            kind: TabularEmitKind::TaskLabels,
        };
        let tasks = Tasks {
            tasks: [
                (TaskIndex(0), "pick apple".to_owned()),
                (TaskIndex(1), "place apple".to_owned()),
            ]
            .into_iter()
            .collect(),
            subtasks: Default::default(),
        };

        let lenses = build_lenses(&[emit], &tasks).unwrap();
        let out = apply(&lenses, &chunk);
        assert_eq!(out.len(), 1, "the raw index column is consumed");
        let chunk = &out[0];
        assert_eq!(chunk.entity_path(), &EntityPath::from("/task"));
        assert!(chunk.timelines().contains_key(&"frame_index".into()));

        let text = chunk
            .components()
            .get(TextDocument::descriptor_text().component)
            .unwrap();
        assert_eq!(text.list_array.len(), 3);
        let values = text.list_array.values();
        let values = values.downcast_array_ref::<StringArray>().unwrap();
        assert_eq!(values.value(0), "pick apple");
        assert!(values.is_null(1), "an index without a label logs nothing");
        assert_eq!(values.value(2), "place apple");
    }

    /// A string feature column becomes a `TextDocument` on its own entity, one string
    /// per row, whatever string encoding the parquet file stored.
    #[test]
    fn string_columns_become_text_documents() {
        let utf8: ArrayRef = Arc::new(StringArray::from(vec![Some("reach"), None, Some("grasp")]));
        let view: ArrayRef = Arc::new(arrow::array::StringViewArray::from(vec![
            Some("reach"),
            None,
            Some("grasp"),
        ]));

        let emit = TabularEmit {
            column: "subtask".to_owned(),
            entity: EntityPath::from("/subtask"),
            kind: TabularEmitKind::Text,
        };
        let lenses = build_lenses(&[emit], &Tasks::default()).unwrap();

        for values in [utf8, view] {
            let out = apply(&lenses, &column_chunk("subtask", values));
            assert_eq!(out.len(), 1, "the raw string column is consumed");
            let chunk = &out[0];
            assert_eq!(chunk.entity_path(), &EntityPath::from("/subtask"));
            assert!(chunk.timelines().contains_key(&"frame_index".into()));

            let text = chunk
                .components()
                .get(TextDocument::descriptor_text().component)
                .unwrap();
            assert_eq!(text.list_array.len(), 3);
            let values = text.list_array.values();
            let values = values.downcast_array_ref::<StringArray>().unwrap();
            assert_eq!(values.value(0), "reach");
            assert!(values.is_null(1), "a null row logs nothing");
            assert_eq!(values.value(2), "grasp");
        }
    }

    /// Chunks whose column has no lens are forwarded unchanged.
    #[test]
    fn unmatched_columns_are_forwarded_unchanged() {
        let values: ArrayRef = Arc::new(arrow::array::Int64Array::from(vec![0_i64, 0]));
        let chunk = column_chunk("language_events", values);

        let lenses = build_lenses(&[], &Tasks::default()).unwrap();
        let out = apply(&lenses, &chunk);
        assert_eq!(out.len(), 1);
        assert!(
            out[0]
                .components()
                .contains_component(ComponentIdentifier::try_new("language_events").unwrap())
        );
    }
}
