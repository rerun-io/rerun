use arrow::array::{Array as _, FixedSizeBinaryArray, ListArray as ArrowListArray};
use arrow::buffer::ScalarBuffer as ArrowScalarBuffer;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_types_core::SerializedComponentColumn;

use crate::chunk::ChunkComponents;
use crate::{Chunk, ChunkError, ChunkId, ChunkResult, TimeColumn};

// ---

impl Chunk {
    /// Picks the order intelligently, and sorts the result.
    pub fn concat_and_sort(left: &Self, right: &Self) -> ChunkResult<Self> {
        re_tracing::profile_function!();

        let left_rowid_min = right.row_id_range().map(|(min, _)| min);
        let right_rowid_min = left.row_id_range().map(|(min, _)| min);
        let mut compacted = if right_rowid_min < left_rowid_min {
            left.concatenated(right)?
        } else {
            right.concatenated(left)?
        };

        compacted.sort_by_row_ids_if_needed();

        // Sanity check that timelines haven't become unsorted.
        // If they have, we have an unsorted timeline, which is good to know about.

        for (name, column) in compacted.timelines() {
            if !column.is_sorted() {
                let left_was_sorted = left.timelines().get(name).is_none_or(|c| c.is_sorted());
                let right_was_sorted = right.timelines().get(name).is_none_or(|c| c.is_sorted());

                if left_was_sorted && right_was_sorted {
                    let entity_path = compacted.entity_path();
                    re_log::debug_warn_once!(
                        "Timeline '{name}' BECAME unsorted after concatenating overlapping, sorted chunks for entity '{entity_path}'. This may cause performance issues."
                    );
                }
            }
        }

        Ok(compacted)
    }

    /// Concatenates two `Chunk`s into a new one.
    ///
    /// The order of the arguments matter: `self`'s contents will precede `rhs`' contents in the
    /// returned `Chunk`.
    ///
    /// This will return an error if the chunks are not [concatenable].
    ///
    /// [concatenable]: [`Chunk::concatenable`]
    pub fn concatenated(&self, rhs: &Self) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "lhs={} rhs={}",
            re_format::format_uint(self.num_rows()),
            re_format::format_uint(rhs.num_rows())
        ));

        let cl = self;
        let cr = rhs;

        if !cl.concatenable(cr) {
            // Make sure we provide good errors:
            let reason = if cl.entity_path() != cr.entity_path() {
                format!(
                    "cannot concatenate chunks with different entity paths: {:?} != {:?}",
                    cl.entity_path(),
                    cr.entity_path()
                )
            } else if !cl.same_timelines(cr) {
                format!(
                    "cannot concatenate chunks with different timelines (timelines are dense within a chunk):\n{:?}\n{:?}",
                    cl.timelines()
                        .values()
                        .map(|column| column.timeline())
                        .sorted()
                        .map(|timeline| format!("{}: {}", timeline.name(), timeline.typ()))
                        .format(", "),
                    cr.timelines()
                        .values()
                        .map(|column| column.timeline())
                        .sorted()
                        .map(|timeline| format!("{}: {}", timeline.name(), timeline.typ()))
                        .format(", "),
                )
            } else if !cl.same_datatypes(cr) {
                format!(
                    "cannot concatenate chunks with different datatypes for shared components:\n{}\n{}",
                    cl.component_descriptors().format(", "),
                    cr.component_descriptors().format(", "),
                )
            } else {
                format!("cannot concatenate incompatible chunks:\n{cl}\n{cr}")
            };
            return Err(ChunkError::Malformed { reason });
        }

        let Some((_cl0, cl1)) = cl.row_id_range() else {
            return Ok(cr.clone()); // `cl` is empty (`cr` might be too, that's fine)
        };
        let Some((cr0, _cr1)) = cr.row_id_range() else {
            return Ok(cl.clone());
        };

        let is_sorted = cl.is_sorted && cr.is_sorted && cl1 <= cr0;

        let row_ids = {
            re_tracing::profile_scope!("row_ids");

            let row_ids = re_arrow_util::concat_arrays(&[&cl.row_ids, &cr.row_ids])?;
            #[expect(clippy::unwrap_used)]
            // concatenating 2 RowId arrays must yield another RowId array
            row_ids
                .downcast_array_ref::<FixedSizeBinaryArray>()
                .unwrap()
                .clone()
        };

        // Pair time columns by name — the maps' iteration orders may differ.
        // Both error arms are unreachable behind the `concatenable` check above; hard-error
        // rather than silently drop a column if that ever changes.
        let timelines: IntMap<_, _> =
            {
                re_tracing::profile_scope!("timelines");
                cl.timelines
                    .iter()
                    .map(|(timeline_name, lhs_time_column)| {
                        let rhs_time_column = cr.timelines.get(timeline_name).ok_or_else(|| {
                            ChunkError::Malformed {
                                reason: format!(
                                    "cannot concatenate chunks: timeline `{timeline_name}` is \
                                     missing from rhs (concatenability should have been checked \
                                     before this point)"
                                ),
                            }
                        })?;
                        let time_column = lhs_time_column
                            .concatenated(rhs_time_column)
                            .ok_or_else(|| ChunkError::Malformed {
                                reason: format!(
                                    "cannot concatenate chunks: timeline `{timeline_name}` differs \
                                     between chunks: {:?} != {:?}",
                                    lhs_time_column.timeline(),
                                    rhs_time_column.timeline(),
                                ),
                            })?;
                        Ok((*timeline_name, time_column))
                    })
                    .collect::<ChunkResult<_>>()?
            };

        let lhs_per_component: IntMap<_, _> = cl
            .components
            .iter()
            .map(|(component, list_array)| (*component, list_array))
            .collect();
        let rhs_per_component: IntMap<_, _> = cr
            .components
            .iter()
            .map(|(component, list_array)| (*component, list_array))
            .collect();

        // First pass: concat right onto left.
        let mut components: ChunkComponents = {
            re_tracing::profile_scope!("components (r2l)");
            lhs_per_component
                .values()
                .filter_map(|lhs_column| {
                    re_tracing::profile_scope!(lhs_column.descriptor.to_string());
                    if let Some(&rhs_column) =
                        rhs_per_component.get(&lhs_column.descriptor.component)
                    {
                        if lhs_column.descriptor != rhs_column.descriptor {
                            re_log::warn_once!("lhs and rhs have different component descriptors for the same component: {} != {}", lhs_column.descriptor, rhs_column.descriptor);
                        }

                        re_tracing::profile_scope!(format!(
                            "concat (lhs={} rhs={})",
                            re_format::format_uint(lhs_column.list_array.values().len()),
                            re_format::format_uint(rhs_column.list_array.values().len()),
                        ));

                        let list_array =
                            re_arrow_util::concat_arrays(&[&lhs_column.list_array, &rhs_column.list_array]).ok()?;
                        let list_array = list_array.downcast_array_ref::<ArrowListArray>()?.clone();

                        Some((lhs_column.descriptor.clone(), list_array))
                    } else {
                        re_tracing::profile_scope!("pad");
                        Some((
                            lhs_column.descriptor.clone(),
                            re_arrow_util::pad_list_array_back(
                                &lhs_column.list_array,
                                self.num_rows() + rhs.num_rows(),
                            ),
                        ))
                    }
                })
                .collect()
        };

        // Second pass: concat left onto right, where necessary.
        {
            re_tracing::profile_scope!("components (l2r)");
            let rhs = rhs_per_component
                .values()
                .filter_map(|rhs_column| {
                    if components.contains_key(&rhs_column.descriptor.component) {
                        // Already did that one during the first pass.
                        return None;
                    }

                    re_tracing::profile_scope!(rhs_column.descriptor.component.to_string());

                    if let Some(&lhs_column) =
                        lhs_per_component.get(&rhs_column.descriptor.component)
                    {
                        if lhs_column.descriptor != rhs_column.descriptor {
                            re_log::warn_once!("lhs and rhs have different component descriptors for the same component: {} != {}", lhs_column.descriptor, rhs_column.descriptor);
                        }

                        re_tracing::profile_scope!(format!(
                            "concat (lhs={} rhs={})",
                            re_format::format_uint(lhs_column.list_array.values().len()),
                            re_format::format_uint(rhs_column.list_array.values().len()),
                        ));

                        let list_array =
                            re_arrow_util::concat_arrays(&[&lhs_column.list_array, &rhs_column.list_array]).ok()?;
                        let list_array = list_array.downcast_array_ref::<ArrowListArray>()?.clone();

                        Some((rhs_column.descriptor.component, SerializedComponentColumn::new(list_array, rhs_column.descriptor.clone())))
                    } else {
                        re_tracing::profile_scope!("pad");
                        Some((
                            rhs_column.descriptor.component,
                            SerializedComponentColumn::new(
                                re_arrow_util::pad_list_array_front(
                                    &rhs_column.list_array,
                                    self.num_rows() + rhs.num_rows(),
                                ),
                                rhs_column.descriptor.clone(),
                            ),
                        ))
                    }
                })
                .collect_vec();
            components.extend(rhs);
        }

        let chunk = Self {
            id: ChunkId::new(),
            entity_path: cl.entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted,
            row_ids,
            timelines,
            components,
        };

        chunk.sanity_check()?;

        Ok(chunk)
    }

    /// Returns `true` if `self` and `rhs` overlap on their `RowId` range.
    #[inline]
    pub fn overlaps_on_row_id(&self, rhs: &Self) -> bool {
        let cl = self;
        let cr = rhs;

        let Some((cl0, cl1)) = cl.row_id_range() else {
            return false;
        };
        let Some((cr0, cr1)) = cr.row_id_range() else {
            return false;
        };

        cl0 <= cr1 && cr0 <= cl1
    }

    /// Returns `true` if `self` and `rhs` overlap on any of their time range(s).
    ///
    /// This does not imply that they share the same exact set of timelines.
    #[inline]
    pub fn overlaps_on_time(&self, rhs: &Self) -> bool {
        self.timelines.iter().any(|(timeline, cl_time_chunk)| {
            if let Some(cr_time_chunk) = rhs.timelines.get(timeline) {
                cl_time_chunk
                    .time_range()
                    .intersects(cr_time_chunk.time_range())
            } else {
                false
            }
        })
    }

    /// Returns `true` if both chunks share the same entity path.
    #[inline]
    pub fn same_entity_paths(&self, rhs: &Self) -> bool {
        self.entity_path() == rhs.entity_path()
    }

    /// Returns `true` if both chunks contain the same set of timelines (both name and type).
    ///
    /// Compared by key lookup — hash-map iteration order differs between maps and means
    /// nothing. Types matter because [`TimeColumn::concatenated`] refuses mismatched types.
    #[inline]
    pub fn same_timelines(&self, rhs: &Self) -> bool {
        self.timelines.len() == rhs.timelines.len()
            && self.timelines.iter().all(|(name, lhs_column)| {
                rhs.timelines
                    .get(name)
                    .is_some_and(|rhs_column| lhs_column.timeline() == rhs_column.timeline())
            })
    }

    /// Returns `true` if both chunks share the same datatypes for the components that
    /// _they have in common_.
    ///
    /// Ignores potential differences in component descriptors.
    #[inline]
    pub fn same_datatypes(&self, rhs: &Self) -> bool {
        self.components.values().all(|lhs_column| {
            if let Some(rhs_column) = rhs.components.get(lhs_column.descriptor.component) {
                lhs_column.list_array.data_type() == rhs_column.list_array.data_type()
            } else {
                true
            }
        })
    }

    /// Returns true if two chunks are concatenable.
    ///
    /// To be concatenable, two chunks must:
    /// * Share the same entity path.
    /// * Share the same exact set of timelines.
    /// * Use the same datatypes for the components they have in common.
    #[inline]
    pub fn concatenable(&self, rhs: &Self) -> bool {
        self.same_entity_paths(rhs) && self.same_timelines(rhs) && self.same_datatypes(rhs)
    }
}

impl TimeColumn {
    /// Concatenates two [`TimeColumn`]s into a new one.
    ///
    /// The order of the arguments matter: `self`'s contents will precede `rhs`' contents in the
    /// returned [`TimeColumn`].
    ///
    /// This will return `None` if the time chunks do not share the same timeline.
    pub fn concatenated(&self, rhs: &Self) -> Option<Self> {
        if self.timeline != rhs.timeline {
            return None;
        }
        re_tracing::profile_function!();

        let is_sorted =
            self.is_sorted && rhs.is_sorted && self.time_range.max() <= rhs.time_range.min();

        let time_range = self.time_range.union(rhs.time_range);

        let times = std::iter::chain(self.times_raw(), rhs.times_raw())
            .copied()
            .collect_vec();
        let times = ArrowScalarBuffer::from(times);

        Some(Self {
            timeline: self.timeline,
            times,
            is_sorted,
            time_range,
        })
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoint64, MyPoints};

    use super::*;
    use crate::{Chunk, RowId, Timeline};

    #[test]
    fn homogeneous() -> anyhow::Result<()> {
        let entity_path = "my/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 1000),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 1032),
            (Timeline::new_sequence("frame"), 3),
        ];
        let timepoint3 = [
            (Timeline::log_time(), 1064),
            (Timeline::new_sequence("frame"), 5),
        ];
        let timepoint4 = [
            (Timeline::log_time(), 1096),
            (Timeline::new_sequence("frame"), 7),
        ];
        let timepoint5 = [
            (Timeline::log_time(), 1128),
            (Timeline::new_sequence("frame"), 9),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[
            MyPoint::new(3.0, 3.0),
            MyPoint::new(4.0, 4.0),
            MyPoint::new(5.0, 5.0),
        ];
        let points5 = &[MyPoint::new(6.0, 7.0)];

        let colors2 = &[MyColor::from_rgb(1, 1, 1)];
        let colors4 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels2 = &[
            MyLabel("a".into()),
            MyLabel("b".into()),
            MyLabel("c".into()),
        ];
        let labels5 = &[MyLabel("d".into())];

        let chunk1 = Chunk::builder(entity_path)
            .with_component_batches(
                row_id1,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_colors(), colors2 as _),
                    (MyPoints::descriptor_labels(), labels2 as _),
                ],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .build()?;

        let chunk2 = Chunk::builder(entity_path)
            .with_component_batches(
                row_id4,
                timepoint4,
                [(MyPoints::descriptor_colors(), colors4 as _)],
            )
            .with_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoints::descriptor_points(), points5 as _),
                    (MyPoints::descriptor_labels(), labels5 as _),
                ],
            )
            .build()?;

        eprintln!("chunk1:\n{chunk1}");
        eprintln!("chunk2:\n{chunk2}");

        {
            assert!(chunk1.concatenable(&chunk2));

            let got = chunk1.concatenated(&chunk2).unwrap();
            let expected = Chunk::builder_with_id(got.id(), entity_path)
                .with_sparse_component_batches(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoints::descriptor_points(), Some(points1 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors2 as _)),
                        (MyPoints::descriptor_labels(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoints::descriptor_points(), Some(points3 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id4,
                    timepoint4,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors4 as _)),
                        (MyPoints::descriptor_labels(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoints::descriptor_points(), Some(points5 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels5 as _)),
                    ],
                )
                .build()?;

            eprintln!("got:\n{got}");
            eprintln!("expected:\n{expected}");

            assert_eq!(
                expected,
                got,
                "{}",
                similar_asserts::SimpleDiff::from_str(
                    &format!("{got}"),
                    &format!("{expected}"),
                    "got",
                    "expected",
                ),
            );

            assert!(got.is_row_ids_sorted());
            assert!(got.all_timelines_sorted());
        }
        {
            assert!(chunk2.concatenable(&chunk1));

            let got = chunk2.concatenated(&chunk1).unwrap();
            let expected = Chunk::builder_with_id(got.id(), entity_path)
                .with_sparse_component_batches(
                    row_id4,
                    timepoint4,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors4 as _)),
                        (MyPoints::descriptor_labels(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoints::descriptor_points(), Some(points5 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels5 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoints::descriptor_points(), Some(points1 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors2 as _)),
                        (MyPoints::descriptor_labels(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoints::descriptor_points(), Some(points3 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), None),
                    ],
                )
                .build()?;

            eprintln!("got:\n{got}");
            eprintln!("expected:\n{expected}");

            assert_eq!(
                expected,
                got,
                "{}",
                similar_asserts::SimpleDiff::from_str(
                    &format!("{got}"),
                    &format!("{expected}"),
                    "got",
                    "expected",
                ),
            );

            assert!(!got.is_row_ids_sorted());
            assert!(!got.all_timelines_sorted());
        }

        Ok(())
    }

    #[test]
    fn heterogeneous() -> anyhow::Result<()> {
        let entity_path = "my/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 1000),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 1032),
            (Timeline::new_sequence("frame"), 3),
        ];
        let timepoint3 = [
            (Timeline::log_time(), 1064),
            (Timeline::new_sequence("frame"), 5),
        ];
        let timepoint4 = [
            (Timeline::log_time(), 1096),
            (Timeline::new_sequence("frame"), 7),
        ];
        let timepoint5 = [
            (Timeline::log_time(), 1128),
            (Timeline::new_sequence("frame"), 9),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(6.0, 7.0)];

        let colors4 = &[MyColor::from_rgb(1, 1, 1)];
        let colors5 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels1 = &[MyLabel("a".into())];
        let labels2 = &[MyLabel("b".into())];
        let labels3 = &[MyLabel("c".into())];
        let labels4 = &[MyLabel("d".into())];
        let labels5 = &[MyLabel("e".into())];

        let chunk1 = Chunk::builder(entity_path)
            .with_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), points1 as _),
                    (MyPoints::descriptor_labels(), labels1 as _),
                ],
            )
            .with_component_batches(
                row_id2,
                timepoint2,
                [(MyPoints::descriptor_labels(), labels2 as _)],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), points3 as _),
                    (MyPoints::descriptor_labels(), labels3 as _),
                ],
            )
            .build()?;

        let chunk2 = Chunk::builder(entity_path)
            .with_component_batches(
                row_id4,
                timepoint4,
                [
                    (MyPoints::descriptor_colors(), colors4 as _),
                    (MyPoints::descriptor_labels(), labels4 as _),
                ],
            )
            .with_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoints::descriptor_colors(), colors5 as _),
                    (MyPoints::descriptor_labels(), labels5 as _),
                ],
            )
            .build()?;

        eprintln!("chunk1:\n{chunk1}");
        eprintln!("chunk2:\n{chunk2}");

        {
            assert!(chunk1.concatenable(&chunk2));

            let got = chunk1.concatenated(&chunk2).unwrap();
            let expected = Chunk::builder_with_id(got.id(), entity_path)
                .with_sparse_component_batches(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoints::descriptor_points(), Some(points1 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoints::descriptor_points(), Some(points3 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels3 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id4,
                    timepoint4,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors4 as _)),
                        (MyPoints::descriptor_labels(), Some(labels4 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors5 as _)),
                        (MyPoints::descriptor_labels(), Some(labels5 as _)),
                    ],
                )
                .build()?;

            eprintln!("got:\n{got}");
            eprintln!("expected:\n{expected}");

            assert_eq!(
                expected,
                got,
                "{}",
                similar_asserts::SimpleDiff::from_str(
                    &format!("{got}"),
                    &format!("{expected}"),
                    "got",
                    "expected",
                ),
            );

            assert!(got.is_row_ids_sorted());
            assert!(got.all_timelines_sorted());
        }
        {
            assert!(chunk2.concatenable(&chunk1));

            let got = chunk2.concatenated(&chunk1).unwrap();
            let expected = Chunk::builder_with_id(got.id(), entity_path)
                .with_sparse_component_batches(
                    row_id4,
                    timepoint4,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors4 as _)),
                        (MyPoints::descriptor_labels(), Some(labels4 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(colors5 as _)),
                        (MyPoints::descriptor_labels(), Some(labels5 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoints::descriptor_points(), Some(points1 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoints::descriptor_points(), Some(points3 as _)),
                        (MyPoints::descriptor_colors(), None),
                        (MyPoints::descriptor_labels(), Some(labels3 as _)),
                    ],
                )
                .build()?;

            eprintln!("got:\n{got}");
            eprintln!("expected:\n{expected}");

            assert_eq!(
                expected,
                got,
                "{}",
                similar_asserts::SimpleDiff::from_str(
                    &format!("{got}"),
                    &format!("{expected}"),
                    "got",
                    "expected",
                ),
            );

            assert!(!got.is_row_ids_sorted());
            assert!(!got.all_timelines_sorted());
        }

        Ok(())
    }

    #[test]
    fn malformed() -> anyhow::Result<()> {
        // Different entity paths
        {
            let entity_path1 = "ent1";
            let entity_path2 = "ent2";

            let row_id1 = RowId::new();
            let row_id2 = RowId::new();

            let timepoint1 = [
                (Timeline::log_time(), 1000),
                (Timeline::new_sequence("frame"), 1),
            ];
            let timepoint2 = [
                (Timeline::log_time(), 1032),
                (Timeline::new_sequence("frame"), 3),
            ];

            let points1 = &[MyPoint::new(1.0, 1.0)];
            let points2 = &[MyPoint::new(2.0, 2.0)];

            let chunk1 = Chunk::builder(entity_path1)
                .with_component_batches(
                    row_id1,
                    timepoint1,
                    [(MyPoints::descriptor_points(), points1 as _)],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path2)
                .with_component_batches(
                    row_id2,
                    timepoint2,
                    [(MyPoints::descriptor_points(), points2 as _)],
                )
                .build()?;

            assert!(matches!(
                chunk1.concatenated(&chunk2),
                Err(ChunkError::Malformed { .. })
            ));
            assert!(matches!(
                chunk2.concatenated(&chunk1),
                Err(ChunkError::Malformed { .. })
            ));
        }

        // Different timelines
        {
            let entity_path = "ent";

            let row_id1 = RowId::new();
            let row_id2 = RowId::new();

            let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
            let timepoint2 = [(Timeline::log_time(), 1032)];

            let points1 = &[MyPoint::new(1.0, 1.0)];
            let points2 = &[MyPoint::new(2.0, 2.0)];

            let chunk1 = Chunk::builder(entity_path)
                .with_component_batches(
                    row_id1,
                    timepoint1,
                    [(MyPoints::descriptor_points(), points1 as _)],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path)
                .with_component_batches(
                    row_id2,
                    timepoint2,
                    [(MyPoints::descriptor_points(), points2 as _)],
                )
                .build()?;

            assert!(matches!(
                chunk1.concatenated(&chunk2),
                Err(ChunkError::Malformed { .. })
            ));
            assert!(matches!(
                chunk2.concatenated(&chunk1),
                Err(ChunkError::Malformed { .. })
            ));
        }

        // Different datatypes
        {
            let entity_path = "ent";

            let row_id1 = RowId::new();
            let row_id2 = RowId::new();

            let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
            let timepoint2 = [(Timeline::new_sequence("frame"), 2)];

            let points32bit =
                <MyPoint as re_types_core::ComponentBatch>::to_arrow(&MyPoint::new(1.0, 1.0))?;
            let points64bit =
                <MyPoint64 as re_types_core::ComponentBatch>::to_arrow(&MyPoint64::new(1.0, 1.0))?;

            let chunk1 = Chunk::builder(entity_path)
                .with_row(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoints::descriptor_points(), points32bit), //
                    ],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path)
                .with_row(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoints::descriptor_points(), points64bit), //
                    ],
                )
                .build()?;

            assert!(matches!(
                chunk1.concatenated(&chunk2),
                Err(ChunkError::Malformed { .. })
            ));
            assert!(matches!(
                chunk2.concatenated(&chunk1),
                Err(ChunkError::Malformed { .. })
            ));
        }

        Ok(())
    }

    /// Rebuild `chunk`'s timeline map by inserting the timelines in `order`.
    ///
    /// Insertion order affects iteration order when keys collide, giving equal key sets that
    /// iterate differently.
    fn with_reinserted_timelines(chunk: &Chunk, order: &[re_log_types::TimelineName]) -> Chunk {
        let mut timelines = IntMap::default();
        for name in order {
            timelines.insert(*name, chunk.timelines[name].clone());
        }
        let mut chunk = chunk.clone();
        chunk.timelines = timelines;
        chunk
    }

    /// Equal timeline sets must concatenate no matter how the maps iterate, pairing time
    /// columns by name.
    ///
    /// Regression test for order-sensitive `same_timelines` and positional pairing in
    /// `concatenated`.
    #[test]
    fn concatenation_is_insensitive_to_timeline_map_iteration_order() -> anyhow::Result<()> {
        use re_log_types::TimelineName;

        // Find names whose map iteration order depends on insertion order. Deterministic
        // (fixed-seed hashes); panics if map internals change and nothing diverges anymore.
        let iteration_order = |insertion_order: &[TimelineName]| {
            let mut set = nohash_hasher::IntSet::<TimelineName>::default();
            for name in insertion_order {
                set.insert(*name);
            }
            set.iter().copied().collect_vec()
        };
        let (order1, order2) = 'search: {
            for i in 0..100 {
                let names = ["a", "b", "c"]
                    .map(|s| TimelineName::try_new(format!("timeline_{s}_{i}")).unwrap());
                let reference = iteration_order(&names);
                for perm in names.iter().copied().permutations(names.len()) {
                    if iteration_order(&perm) != reference {
                        break 'search (names.to_vec(), perm);
                    }
                }
            }
            panic!(
                "no timeline-name set found whose IntMap iteration order depends on insertion \
                 order; did the hasher or hash-map internals change?"
            );
        };

        // Distinct time values per timeline per chunk, so wrongly paired columns can't match by
        // accident.
        let entity_path = "my/entity";
        let timepoint = |chunk_index: i64, row: i64| -> [(Timeline, i64); 3] {
            std::array::from_fn(|k| {
                let timeline_index = i64::try_from(k).expect("tiny index");
                (
                    Timeline::new_sequence(order1[k]),
                    1000 * (timeline_index + 1) + 10 * chunk_index + row,
                )
            })
        };
        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let build_chunk = |chunk_index: i64, points: &dyn re_types_core::ComponentBatch| {
            Chunk::builder(entity_path)
                .with_component_batches(
                    RowId::new(),
                    timepoint(chunk_index, 0),
                    [(MyPoints::descriptor_points(), points)],
                )
                .with_component_batches(
                    RowId::new(),
                    timepoint(chunk_index, 1),
                    [(MyPoints::descriptor_points(), points)],
                )
                .build()
        };
        let chunk1 = build_chunk(0, points1 as _)?;
        let chunk2 = build_chunk(1, points2 as _)?;

        // Force divergent map layouts; assert the precondition actually holds.
        let chunk1 = with_reinserted_timelines(&chunk1, &order1);
        let chunk2 = with_reinserted_timelines(&chunk2, &order2);
        assert_ne!(
            chunk1.timelines.keys().collect_vec(),
            chunk2.timelines.keys().collect_vec(),
            "test precondition: the two timeline maps must iterate in different orders"
        );

        assert!(chunk1.same_timelines(&chunk2));
        assert!(chunk1.concatenable(&chunk2));

        let got = chunk1.concatenated(&chunk2)?;

        // Paired by name, not map position.
        for name in &order1 {
            let expected: Vec<i64> = std::iter::chain(
                chunk1.timelines[name].times_raw(),
                chunk2.timelines[name].times_raw(),
            )
            .copied()
            .collect();
            assert_eq!(
                got.timelines()[name].times_raw(),
                expected.as_slice(),
                "timeline {name}"
            );
        }
        got.sanity_check()?;

        Ok(())
    }
}
