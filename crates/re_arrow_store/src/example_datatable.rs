/// Crafts a simple but interesting [`DataTable`].
#[cfg(not(target_arch = "wasm32"))]
pub fn example_datatable(timeless: bool) -> re_log_types::DataTable {
    use re_log_types::{DataRow, DataTable, RowId, TableId, Time, TimePoint, Timeline};
    use re_types::components::{Color, Position2D, Text};

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
        let positions: &[Position2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
        let colors: &[_] = &[Color::from_rgb(128, 128, 128)];
        let labels: &[Text] = &[];

        DataRow::from_cells3(
            RowId::random(),
            "a",
            timepoint(1),
            num_instances,
            (positions, colors, labels),
        )
        .unwrap()
    };

    let row1 = {
        let num_instances = 0;
        let colors: &[Color] = &[];

        DataRow::from_cells1(RowId::random(), "b", timepoint(1), num_instances, colors).unwrap()
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
        .unwrap()
    };

    let mut table = DataTable::from_rows(table_id, [row0, row1, row2]);
    table.compute_all_size_bytes();

    table
}
