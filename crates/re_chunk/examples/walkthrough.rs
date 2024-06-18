#![allow(clippy::unwrap_used)]

use itertools::Itertools;
use re_chunk::{Chunk, LatestAtQuery, RangeQuery, RowId, TimeInt, TimePoint, Timeline};
use re_log_types::{
    example_components::{MyColor, MyLabel, MyPoint},
    ResolvedTimeRange, Time,
};
use re_types_core::{Component, Loggable as _};

// ---

fn main() -> anyhow::Result<()> {
    let frame_nr = Timeline::new_sequence("frame_nr");
    let mut step = 0;

    let mut chunk = create_chunk(10)?;
    step += 1;
    eprintln!("\n\n{step}) Random data:\n{chunk}");

    chunk.sort_if_unsorted();
    step += 1;
    eprintln!("\n\n{step}) RowId ordered:\n{chunk}");

    {
        let raw = chunk.components().get(&MyPoint::name()).unwrap();
        step += 1;
        eprintln!(
            "\n\n{step}) Raw full column:\n    {raw:?}\n    {:?}",
            raw.values()
        );
    }

    {
        let chunk = chunk
            .timeline_sliced(Timeline::log_tick())
            .components_sliced(&[MyColor::name(), MyLabel::name()].into_iter().collect())
            .row_sliced(3 /* offset */, 3 /* len */);

        step += 1;
        eprintln!("\n\n{step}) Sliced:\n{chunk}");
    }

    {
        let densified = chunk.densified(MyLabel::name());

        step += 1;
        eprintln!("\n\n{step}) Densified:\n{densified}");

        let raw = chunk.components().get(&MyLabel::name()).unwrap();
        eprintln!("Raw access before: {raw:?} >>> {:?}", raw.values());

        let raw = densified.components().get(&MyLabel::name()).unwrap();
        eprintln!("Raw access after: {raw:?} >>> {:?}", raw.values());
    }

    let chunk = chunk.sorted_by_timeline_if_unsorted(&frame_nr);
    step += 1;
    eprintln!("\n\n{step}) Sorted by `frame_nr`:\n{chunk}");

    {
        let query = LatestAtQuery::latest(frame_nr);
        let chunk_points = chunk.latest_at(&query, MyPoint::name());
        let chunk_colors = chunk.latest_at(&query, MyColor::name());
        let chunk_labels = chunk.latest_at(&query, MyLabel::name());

        step += 1;
        eprintln!("\n\n{step}) Results for {query:?}:\nPoints:\n{chunk_points}\nColors:\n{chunk_colors}\nLabels:\n{chunk_labels}");
    }

    let query = RangeQuery::new(frame_nr, ResolvedTimeRange::new(2, 4));
    let chunk_points = chunk.range(&query, MyPoint::name());
    let chunk_colors = chunk.range(&query, MyColor::name());
    let chunk_labels = chunk.range(&query, MyLabel::name());

    step += 1;
    eprintln!("\n\n{step}) {query:?}:\nPoints:\n{chunk_points}\nColors:\n{chunk_colors}\nLabels:\n{chunk_labels}");

    let point_values = chunk_points.iter_indexed::<MyPoint>(&frame_nr);
    let color_values = chunk_colors.iter_indexed::<MyColor>(&frame_nr);
    let label_values = chunk_labels.iter_indexed::<MyLabel>(&frame_nr);
    step += 1;
    eprintln!("\n\n{step}) Deserialized results for {query:?}:");
    eprintln!("MyPoint: [{}\n]", print_indexed(point_values));
    eprintln!("MyColor: [{}\n]", print_indexed(color_values));
    eprintln!("MyLabel: [{}\n]", print_indexed(label_values));

    let point_values = chunk_points.iter_indexed::<MyPoint>(&frame_nr);
    let color_values = chunk_colors.iter_indexed::<MyColor>(&frame_nr);
    let label_values = chunk_labels.iter_indexed::<MyLabel>(&frame_nr);
    let results = range_zip::range_zip_1x2(point_values, color_values, label_values)
        .map(|(index, points, colors, labels)| (index, (points, colors, labels)));
    step += 1;
    eprintln!(
        "\n\n{step}) Zipped results for {query:?}:\n[{}\n]",
        print_indexed(results)
    );

    Ok(())
}

// --

fn create_chunk(num_rows: usize) -> anyhow::Result<Chunk> {
    use rand::seq::SliceRandom as _;
    use rand::{rngs::ThreadRng, Rng};

    let mut rng = rand::thread_rng();

    let mut row_ids = {
        let mut row_ids = std::iter::from_fn({
            let mut row_id = RowId::new();
            move || {
                row_id = row_id.next();
                Some(row_id)
            }
        })
        .take(num_rows)
        .collect_vec();

        row_ids.shuffle(&mut rng);

        row_ids
    };

    let now = Time::now().nanos_since_epoch();
    let possible_timelines = [
        (
            Timeline::log_time(),
            now..now + num_rows as i64 * 10 * 1e9 as i64,
            false,
        ),
        (Timeline::log_tick(), 0..num_rows as i64 * 10, true),
        (
            Timeline::new_sequence("frame_nr"),
            0..num_rows as i64 / 2,
            false,
        ),
    ];

    let generate_points = |rng: &mut ThreadRng| {
        let num_instances = rng.gen_range(0..3);
        std::iter::from_fn(|| {
            let xy = rng.gen_range(0..=100) as f32;
            Some(MyPoint::new(xy, xy))
        })
        .take(num_instances)
        .collect_vec()
    };

    let generate_colors = |rng: &mut ThreadRng| {
        let num_instances = rng.gen_range(0..3);
        std::iter::from_fn(|| {
            let rgb = rng.gen_range(0..=255) as u8;
            Some(MyColor::from_rgb(rgb, rgb, rgb))
        })
        .take(num_instances)
        .collect_vec()
    };

    let generate_labels = |rng: &mut ThreadRng| {
        let num_instances = rng.gen_range(0..3);
        std::iter::from_fn(|| {
            let n = rng.gen_range(0..=25) as u32;
            let c = char::from_u32('a' as u32 + n).unwrap();
            Some(MyLabel(c.to_string()))
        })
        .take(num_instances)
        .collect_vec()
    };

    let mut chunk = Chunk::builder("my/entity".into());

    for i in 0..num_rows {
        let row_id = row_ids.pop().unwrap();

        let mut timepoint = TimePoint::default();
        for (timeline, time_range, sorted) in possible_timelines.clone() {
            let time = if sorted {
                time_range.get(i..i + 1).next().unwrap()
            } else {
                rng.gen_range(time_range)
            };
            timepoint.insert(timeline, time);
        }

        let points = generate_points(&mut rng);
        let colors = generate_colors(&mut rng);
        let labels = generate_labels(&mut rng);

        chunk = chunk.with_sparse_component_batches(
            row_id,
            timepoint,
            [
                (MyPoint::name(), rng.gen_bool(0.9).then_some(&points as _)), //
                (MyColor::name(), rng.gen_bool(0.6).then_some(&colors as _)), //
                (MyLabel::name(), rng.gen_bool(0.3).then_some(&labels as _)), //
            ],
        );
    }

    let chunk = chunk.build_with_datatypes(
        &[
            (MyPoint::name(), MyPoint::arrow_datatype()), //
            (MyColor::name(), MyColor::arrow_datatype()), //
            (MyLabel::name(), MyLabel::arrow_datatype()), //
        ]
        .into_iter()
        .collect(),
    )?;

    Ok(chunk)
}

fn print_indexed<'a, C: std::fmt::Debug>(
    it: impl Iterator<Item = ((TimeInt, RowId), C)>,
) -> String {
    let mut strs = Vec::new();

    for ((data_time, row_id), values) in it {
        strs.push(format!(
            "\n    ((#{}_{}), {values:?})",
            data_time.as_i64(),
            row_id.short_string()
        ))
    }

    strs.join("")
}

// ---

mod range_zip {
    #![allow(clippy::iter_on_single_items)]
    #![allow(clippy::too_many_arguments)]
    #![allow(clippy::type_complexity)]

    use std::iter::Peekable;

    /// Returns a new [`RangeZip1x2`] iterator.
    ///
    /// The number of elements in a range zip iterator corresponds to the number of elements in the
    /// shortest of its required iterators (`r0`).
    ///
    /// Each call to `next` is guaranteed to yield the next value for each required iterator,
    /// as well as the most recent index amongst all of them.
    ///
    /// Optional iterators accumulate their state and yield their most recent value (if any),
    /// each time the required iterators fire.
    pub fn range_zip_1x2<Idx, IR0, R0, IO0, O0, IO1, O1>(
        r0: IR0,
        o0: IO0,
        o1: IO1,
    ) -> RangeZip1x2<Idx, IR0::IntoIter, R0, IO0::IntoIter, O0, IO1::IntoIter, O1>
    where
        Idx: std::cmp::Ord,
        IR0: IntoIterator<Item = (Idx, R0)>,
        IO0: IntoIterator<Item = (Idx, O0)>,
        IO1: IntoIterator<Item = (Idx, O1)>,
    {
        RangeZip1x2 {
            r0: r0.into_iter(),
            o0: o0.into_iter().peekable(),
            o1: o1.into_iter().peekable(),

            o0_data_latest: None,
            o1_data_latest: None,
        }
    }

    /// Implements a range zip iterator combinator with 2 required iterators and 2 optional
    /// iterators.
    ///
    /// See [`range_zip_1x2`] for more information.
    pub struct RangeZip1x2<Idx, IR0, R0, IO0, O0, IO1, O1>
    where
        Idx: std::cmp::Ord,
        IR0: Iterator<Item = (Idx, R0)>,
        IO0: Iterator<Item = (Idx, O0)>,
        IO1: Iterator<Item = (Idx, O1)>,
    {
        r0: IR0,
        o0: Peekable<IO0>,
        o1: Peekable<IO1>,

        o0_data_latest: Option<O0>,
        o1_data_latest: Option<O1>,
    }

    impl<Idx, IR0, R0, IO0, O0, IO1, O1> Iterator for RangeZip1x2<Idx, IR0, R0, IO0, O0, IO1, O1>
    where
        Idx: std::cmp::Ord,
        IR0: Iterator<Item = (Idx, R0)>,
        IO0: Iterator<Item = (Idx, O0)>,
        IO1: Iterator<Item = (Idx, O1)>,
        O0: Clone,
        O1: Clone,
    {
        type Item = (Idx, R0, Option<O0>, Option<O1>);

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            let Self {
                r0,
                o0,
                o1,
                o0_data_latest,
                o1_data_latest,
            } = self;

            let (r0_index, r0_data) = r0.next()?;

            let max_index = [r0_index].into_iter().max()?;

            let mut o0_data = None;
            while let Some((_, data)) = o0.next_if(|(index, _)| index <= &max_index) {
                o0_data = Some(data);
            }
            let o0_data = o0_data.or(o0_data_latest.take());
            o0_data_latest.clone_from(&o0_data);

            let mut o1_data = None;
            while let Some((_, data)) = o1.next_if(|(index, _)| index <= &max_index) {
                o1_data = Some(data);
            }
            let o1_data = o1_data.or(o1_data_latest.take());
            o1_data_latest.clone_from(&o1_data);

            Some((max_index, r0_data, o0_data, o1_data))
        }
    }
}
