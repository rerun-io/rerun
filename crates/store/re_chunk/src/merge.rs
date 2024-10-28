use std::collections::BTreeMap;

use arrow2::array::{
    Array as ArrowArray, ListArray as ArrowListArray, PrimitiveArray as ArrowPrimitiveArray,
    StructArray as ArrowStructArray,
};
use itertools::{izip, Itertools};

use crate::{Chunk, ChunkError, ChunkId, ChunkResult, TimeColumn};

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

            let row_ids = crate::util::concat_arrays(&[&cl.row_ids, &cr.row_ids])?;
            #[allow(clippy::unwrap_used)]
            // concatenating 2 RowId arrays must yield another RowId array
            row_ids
                .as_any()
                .downcast_ref::<ArrowStructArray>()
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

        // First pass: concat right onto left.
        let mut components: BTreeMap<_, _> = {
            re_tracing::profile_scope!("components (r2l)");
            self.components
                .iter()
                .filter_map(|(component_name, lhs_list_array)| {
                    re_tracing::profile_scope!(format!("{}", component_name.as_str()));
                    if let Some(rhs_list_array) = rhs.components.get(component_name) {
                        re_tracing::profile_scope!(format!(
                            "concat (lhs={} rhs={})",
                            re_format::format_uint(lhs_list_array.values().len()),
                            re_format::format_uint(rhs_list_array.values().len()),
                        ));

                        let list_array =
                            crate::util::concat_arrays(&[lhs_list_array, rhs_list_array]).ok()?;
                        let list_array = list_array
                            .as_any()
                            .downcast_ref::<ArrowListArray<i32>>()?
                            .clone();

                        Some((*component_name, list_array))
                    } else {
                        re_tracing::profile_scope!("pad");
                        Some((
                            *component_name,
                            crate::util::pad_list_array_back(
                                lhs_list_array,
                                self.num_rows() + rhs.num_rows(),
                            ),
                        ))
                    }
                })
                .collect()
        };

        // Second pass: concat left onto right, where necessary.
        components.extend({
            re_tracing::profile_scope!("components (l2r)");
            rhs.components
                .iter()
                .filter_map(|(component_name, rhs_list_array)| {
                    if components.contains_key(component_name) {
                        // Already did that one during the first pass.
                        return None;
                    }

                    re_tracing::profile_scope!(component_name.as_str());

                    if let Some(lhs_list_array) = self.components.get(component_name) {
                        re_tracing::profile_scope!(format!(
                            "concat (lhs={} rhs={})",
                            re_format::format_uint(lhs_list_array.values().len()),
                            re_format::format_uint(rhs_list_array.values().len()),
                        ));

                        let list_array =
                            crate::util::concat_arrays(&[lhs_list_array, rhs_list_array]).ok()?;
                        let list_array = list_array
                            .as_any()
                            .downcast_ref::<ArrowListArray<i32>>()?
                            .clone();

                        Some((*component_name, list_array))
                    } else {
                        re_tracing::profile_scope!("pad");
                        Some((
                            *component_name,
                            crate::util::pad_list_array_front(
                                rhs_list_array,
                                self.num_rows() + rhs.num_rows(),
                            ),
                        ))
                    }
                })
                .collect_vec()
        });

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
            .all(|(component_name, lhs_list_array)| {
                if let Some(rhs_list_array) = rhs.components.get(component_name) {
                    lhs_list_array.data_type() == rhs_list_array.data_type()
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

        let is_sorted =
            self.is_sorted && rhs.is_sorted && self.time_range.max() <= rhs.time_range.min();

        let time_range = self.time_range.union(rhs.time_range);

        let times = crate::util::concat_arrays(&[&self.times, &rhs.times]).ok()?;
        let times = times
            .as_any()
            .downcast_ref::<ArrowPrimitiveArray<i64>>()?
            .clone();

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

    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoint64};
    use re_types_core::Loggable;

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
            .with_component_batches(row_id1, timepoint1, [points1 as _])
            .with_component_batches(row_id2, timepoint2, [colors2 as _, labels2 as _])
            .with_component_batches(row_id3, timepoint3, [points3 as _])
            .build()?;

        let chunk2 = Chunk::builder(entity_path.into())
            .with_component_batches(row_id4, timepoint4, [colors4 as _])
            .with_component_batches(row_id5, timepoint5, [points5 as _, labels5 as _])
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
                        (MyPoint::name(), Some(points1 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors2 as _)),
                        (MyLabel::name(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoint::name(), Some(points3 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id4,
                    timepoint4,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors4 as _)),
                        (MyLabel::name(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoint::name(), Some(points5 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels5 as _)),
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
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors4 as _)),
                        (MyLabel::name(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoint::name(), Some(points5 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels5 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoint::name(), Some(points1 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors2 as _)),
                        (MyLabel::name(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoint::name(), Some(points3 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), None),
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
            .with_component_batches(row_id1, timepoint1, [points1 as _, labels1 as _])
            .with_component_batches(row_id2, timepoint2, [labels2 as _])
            .with_component_batches(row_id3, timepoint3, [points3 as _, labels3 as _])
            .build()?;

        let chunk2 = Chunk::builder(entity_path.into())
            .with_component_batches(row_id4, timepoint4, [colors4 as _, labels4 as _])
            .with_component_batches(row_id5, timepoint5, [colors5 as _, labels5 as _])
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
                        (MyPoint::name(), Some(points1 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoint::name(), Some(points3 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels3 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id4,
                    timepoint4,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors4 as _)),
                        (MyLabel::name(), Some(labels4 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors5 as _)),
                        (MyLabel::name(), Some(labels5 as _)),
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
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors4 as _)),
                        (MyLabel::name(), Some(labels4 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id5,
                    timepoint5,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), Some(colors5 as _)),
                        (MyLabel::name(), Some(labels5 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id1,
                    timepoint1,
                    [
                        (MyPoint::name(), Some(points1 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoint::name(), None),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    timepoint3,
                    [
                        (MyPoint::name(), Some(points3 as _)),
                        (MyColor::name(), None),
                        (MyLabel::name(), Some(labels3 as _)),
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
                .with_component_batches(row_id1, timepoint1, [points1 as _])
                .build()?;

            let chunk2 = Chunk::builder(entity_path2.into())
                .with_component_batches(row_id2, timepoint2, [points2 as _])
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
                .with_component_batches(row_id1, timepoint1, [points1 as _])
                .build()?;

            let chunk2 = Chunk::builder(entity_path.into())
                .with_component_batches(row_id2, timepoint2, [points2 as _])
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
                        (MyPoint::name(), points32bit), //
                    ],
                )
                .build()?;

            let chunk2 = Chunk::builder(entity_path.into())
                .with_row(
                    row_id2,
                    timepoint2,
                    [
                        (MyPoint::name(), points64bit), //
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
