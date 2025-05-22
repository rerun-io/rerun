use arrow::array::FixedSizeBinaryArray;
use arrow::array::{Array as _, ListArray as ArrowListArray};
use arrow::buffer::ScalarBuffer as ArrowScalarBuffer;
use itertools::{Itertools as _, izip};
use nohash_hasher::IntMap;

use re_arrow_util::ArrowArrayDowncastRef as _;

use crate::{Chunk, ChunkError, ChunkId, ChunkResult, TimeColumn, chunk::ChunkComponents};

// ---

impl Chunk {
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
            return Err(ChunkError::Malformed {
                reason: format!("cannot concatenate incompatible Chunks:\n{cl}\n{cr}"),
            });
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
            #[allow(clippy::unwrap_used)]
            // concatenating 2 RowId arrays must yield another RowId array
            row_ids
                .downcast_array_ref::<FixedSizeBinaryArray>()
                .unwrap()
                .clone()
        };

        // NOTE: We know they are the same set, and they are in a btree => we can zip them.
        let timelines = {
            re_tracing::profile_scope!("timelines");
            izip!(self.timelines.iter(), rhs.timelines.iter())
                .filter_map(
                    |((lhs_timeline, lhs_time_chunk), (rhs_timeline, rhs_time_chunk))| {
                        debug_assert_eq!(lhs_timeline, rhs_timeline);
                        lhs_time_chunk
                            .concatenated(rhs_time_chunk)
                            .map(|time_column| (*lhs_timeline, time_column))
                    },
                )
                .collect()
        };

        let lhs_per_desc: IntMap<_, _> = cl
            .components
            .iter()
            .map(|(component_desc, list_array)| (component_desc.clone(), list_array))
            .collect();
        let rhs_per_desc: IntMap<_, _> = cr
            .components
            .iter()
            .map(|(component_desc, list_array)| (component_desc.clone(), list_array))
            .collect();

        // First pass: concat right onto left.
        let mut components = ChunkComponents({
            re_tracing::profile_scope!("components (r2l)");
            lhs_per_desc
                .iter()
                .filter_map(|(component_desc, &lhs_list_array)| {
                    re_tracing::profile_scope!(component_desc.to_string());
                    if let Some(&rhs_list_array) = rhs_per_desc.get(component_desc) {
                        re_tracing::profile_scope!(format!(
                            "concat (lhs={} rhs={})",
                            re_format::format_uint(lhs_list_array.values().len()),
                            re_format::format_uint(rhs_list_array.values().len()),
                        ));

                        let list_array =
                            re_arrow_util::concat_arrays(&[lhs_list_array, rhs_list_array]).ok()?;
                        let list_array = list_array.downcast_array_ref::<ArrowListArray>()?.clone();

                        Some((component_desc.clone(), list_array))
                    } else {
                        re_tracing::profile_scope!("pad");
                        Some((
                            component_desc.clone(),
                            re_arrow_util::pad_list_array_back(
                                lhs_list_array,
                                self.num_rows() + rhs.num_rows(),
                            ),
                        ))
                    }
                })
                .collect()
        });

        // Second pass: concat left onto right, where necessary.
        {
            re_tracing::profile_scope!("components (l2r)");
            let rhs = rhs_per_desc
                .iter()
                .filter_map(|(component_desc, &rhs_list_array)| {
                    if components.contains_key(component_desc) {
                        // Already did that one during the first pass.
                        return None;
                    }

                    re_tracing::profile_scope!(component_desc.to_string());

                    if let Some(&lhs_list_array) = lhs_per_desc.get(component_desc) {
                        re_tracing::profile_scope!(format!(
                            "concat (lhs={} rhs={})",
                            re_format::format_uint(lhs_list_array.values().len()),
                            re_format::format_uint(rhs_list_array.values().len()),
                        ));

                        let list_array =
                            re_arrow_util::concat_arrays(&[lhs_list_array, rhs_list_array]).ok()?;
                        let list_array = list_array.downcast_array_ref::<ArrowListArray>()?.clone();

                        Some((component_desc.clone(), list_array))
                    } else {
                        re_tracing::profile_scope!("pad");
                        Some((
                            component_desc.clone(),
                            re_arrow_util::pad_list_array_front(
                                rhs_list_array,
                                self.num_rows() + rhs.num_rows(),
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

    /// Returns `true` if both chunks contains the same set of timelines.
    #[inline]
    pub fn same_timelines(&self, rhs: &Self) -> bool {
        self.timelines.len() == rhs.timelines.len()
            && self.timelines.keys().collect_vec() == rhs.timelines.keys().collect_vec()
    }

    /// Returns `true` if both chunks share the same datatypes for the components that
    /// _they have in common_.
    #[inline]
    pub fn same_datatypes(&self, rhs: &Self) -> bool {
        self.components
            .iter()
            .all(|(component_desc, lhs_list_array)| {
                if let Some(rhs_list_array) = rhs.components.get(component_desc) {
                    lhs_list_array.data_type() == rhs_list_array.data_type()
                } else {
                    true
                }
            })
    }

    /// Returns `true` if both chunks share the same descriptors for the components that
    /// _they have in common_.
    #[inline]
    pub fn same_descriptors(&self, rhs: &Self) -> bool {
        self.components.keys().all(|lhs_desc| {
            if rhs.components.contains_key(lhs_desc) {
                true
            } else {
                rhs.components
                    .get_by_component_name(lhs_desc.component_name)
                    .next()
                    .is_none()
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
        self.same_entity_paths(rhs)
            && self.same_timelines(rhs)
            && self.same_datatypes(rhs)
            && self.same_descriptors(rhs)
    }

    /// Moves all indicator components from `self` into a new, dedicated chunk.
    ///
    /// The new chunk contains only the first index from each index column, and all the indicators,
    /// packed in a single row.
    /// Beware: `self` might be left with no component columns at all after this operation.
    ///
    /// This greatly reduces the overhead of indicators, both in the row-oriented and
    /// column-oriented APIs.
    /// See <https://github.com/rerun-io/rerun/issues/8768> for further rationale.
    pub fn split_indicators(&mut self) -> Option<Self> {
        let indicators: ChunkComponents = self
            .components
            .iter()
            .filter(|&(descr, _list_array)| descr.component_name.is_indicator_component())
            .filter(|&(_descr, list_array)| (!list_array.is_empty()))
            .map(|(descr, list_array)| (descr.clone(), list_array.slice(0, 1)))
            .collect();
        if indicators.is_empty() {
            return None;
        }

        let timelines = self
            .timelines
            .iter()
            .map(|(timeline, time_column)| (*timeline, time_column.row_sliced(0, 1)))
            .collect();

        if let Ok(chunk) = Self::from_auto_row_ids(
            ChunkId::new(),
            self.entity_path.clone(),
            timelines,
            indicators,
        ) {
            self.components
                .retain(|desc, _per_desc| !desc.component_name.is_indicator_component());
            return Some(chunk);
        }

        None
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

        let times = self
            .times_raw()
            .iter()
            .chain(rhs.times_raw())
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
    use super::*;

    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoint64, MyPoints};

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

        let chunk1 = Chunk::builder(entity_path.into())
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

        let chunk2 = Chunk::builder(entity_path.into())
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
            let expected = Chunk::builder_with_id(got.id(), entity_path.into())
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

            assert!(got.is_sorted());
            assert!(got.is_time_sorted());
        }
        {
            assert!(chunk2.concatenable(&chunk1));

            let got = chunk2.concatenated(&chunk1).unwrap();
            let expected = Chunk::builder_with_id(got.id(), entity_path.into())
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

            assert!(!got.is_sorted());
            assert!(!got.is_time_sorted());
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

        let chunk1 = Chunk::builder(entity_path.into())
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

        let chunk2 = Chunk::builder(entity_path.into())
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
            let expected = Chunk::builder_with_id(got.id(), entity_path.into())
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

            assert!(got.is_sorted());
            assert!(got.is_time_sorted());
        }
        {
            assert!(chunk2.concatenable(&chunk1));

            let got = chunk2.concatenated(&chunk1).unwrap();
            let expected = Chunk::builder_with_id(got.id(), entity_path.into())
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

            assert!(!got.is_sorted());
            assert!(!got.is_time_sorted());
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

            let chunk1 = Chunk::builder(entity_path1.into())
                .with_component_batches(
                    row_id1,
                    timepoint1,
                    [(MyPoints::descriptor_points(), points1 as _)],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path2.into())
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

            let chunk1 = Chunk::builder(entity_path.into())
                .with_component_batches(
                    row_id1,
                    timepoint1,
                    [(MyPoints::descriptor_points(), points1 as _)],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path.into())
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
                <MyPoint as re_types_core::LoggableBatch>::to_arrow(&MyPoint::new(1.0, 1.0))?;
            let points64bit =
                <MyPoint64 as re_types_core::LoggableBatch>::to_arrow(&MyPoint64::new(1.0, 1.0))?;

            let chunk1 = Chunk::builder(entity_path.into())
                .with_row(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoints::descriptor_points(), points32bit), //
                    ],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path.into())
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
}
