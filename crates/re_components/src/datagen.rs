//! Generate random data for tests and benchmarks.

// TODO(#1810): It really is time for whole module to disappear.

use re_log_types::{Time, TimeInt, TimeType, Timeline};
use re_types::components::InstanceKey;

/// Create `len` dummy colors
pub fn build_some_colors(len: usize) -> Vec<re_types::components::Color> {
    (0..len)
        .map(|i| re_types::components::Color::from(i as u32))
        .collect()
}

/// Create `len` dummy `Point2D`
pub fn build_some_point2d(len: usize) -> Vec<crate::Point2D> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    (0..len)
        .map(|_| crate::Point2D::new(rng.gen_range(0.0..10.0), rng.gen_range(0.0..10.0)))
        .collect()
}

/// Create `len` dummy `Vec3D`
pub fn build_some_vec3d(len: usize) -> Vec<crate::LegacyVec3D> {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    (0..len)
        .map(|_| {
            crate::LegacyVec3D::new(
                rng.gen_range(0.0..10.0),
                rng.gen_range(0.0..10.0),
                rng.gen_range(0.0..10.0),
            )
        })
        .collect()
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`re_log_types::TimePoint`].
pub fn build_log_time(log_time: Time) -> (Timeline, TimeInt) {
    (Timeline::log_time(), log_time.into())
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`re_log_types::TimePoint`].
pub fn build_frame_nr(frame_nr: TimeInt) -> (Timeline, TimeInt) {
    (Timeline::new("frame_nr", TimeType::Sequence), frame_nr)
}

/// Create `len` dummy `InstanceKey` keys. These keys will be sorted.
pub fn build_some_instances(num_instances: usize) -> Vec<InstanceKey> {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();

    // Allocate pool of 10x the potential instance keys, draw a random sampling, and then sort it
    let mut instance_pool = (0..(num_instances * 10)).collect::<Vec<_>>();
    let (rand_instances, _) = instance_pool.partial_shuffle(&mut rng, num_instances);
    let mut sorted_instances = rand_instances.to_vec();
    sorted_instances.sort();

    sorted_instances
        .into_iter()
        .map(|id| InstanceKey(id as u64))
        .collect()
}

pub fn build_some_instances_from(instances: impl IntoIterator<Item = u64>) -> Vec<InstanceKey> {
    let mut instances = instances.into_iter().map(InstanceKey).collect::<Vec<_>>();
    instances.sort();
    instances
}

/// Crafts a simple but interesting [`re_log_types::DataTable`].
#[cfg(not(target_arch = "wasm32"))]
pub fn data_table_example(timeless: bool) -> re_log_types::DataTable {
    use re_log_types::{DataRow, DataTable, RowId, TableId, TimePoint};
    use re_types::components::{Color, Point2D, Text};

    let table_id = TableId::random();

    let mut tick = 0i64;
    let mut timepoint = |frame_nr: i64| {
        let tp = if timeless {
            TimePoint::timeless()
        } else {
            TimePoint::from([
                (Timeline::log_time(), Time::now().into()),
                (Timeline::log_tick(), tick.into()),
                (Timeline::new_sequence("frame_nr"), frame_nr.into()),
            ])
        };
        tick += 1;
        tp
    };

    let row0 = {
        let num_instances = 2;
        let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
        let colors: &[_] = &[Color::from_rgb(128, 128, 128)];
        let labels: &[Text] = &[];

        DataRow::from_cells3(
            RowId::random(),
            "a",
            timepoint(1),
            num_instances,
            (points, colors, labels),
        )
    };

    let row1 = {
        let num_instances = 0;
        let colors: &[Color] = &[];

        DataRow::from_cells1(RowId::random(), "b", timepoint(1), num_instances, colors)
    };

    let row2 = {
        let num_instances = 1;
        let colors: &[_] = &[Color::from_rgb(255, 255, 255)];
        let labels: &[_] = &[Text("hey".into())];

        DataRow::from_cells2(
            RowId::random(),
            "c",
            timepoint(2),
            num_instances,
            (colors, labels),
        )
    };

    let mut table = DataTable::from_rows(table_id, [row0, row1, row2]);
    table.compute_all_size_bytes();

    table
}
