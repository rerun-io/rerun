use crossbeam::{channel::TryRecvError, select};
use itertools::Itertools as _;

use re_log_types::{
    DataRow, DataTableBatcher, DataTableBatcherConfig, RowId, SizeBytes, TimePoint, Timeline,
};
use re_log_types::{DataTable, TableId, Time};
use re_types::components::{Color, Position2D, Text};

#[test]
fn manual_trigger() {
    let batcher = DataTableBatcher::new(DataTableBatcherConfig::NEVER).unwrap();
    let tables = batcher.tables();

    let mut expected = create_table();
    expected.compute_all_size_bytes();

    for _ in 0..3 {
        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        for row in expected.try_to_rows() {
            batcher.push_row(row.unwrap());
        }

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        batcher.flush_blocking();

        {
            let mut table = tables.recv().unwrap();
            // NOTE: Override the resulting table's ID so they can be compared.
            table.table_id = expected.table_id;

            similar_asserts::assert_eq!(expected, table);
        }

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());
    }

    drop(batcher);

    assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
}

#[test]
fn shutdown_trigger() {
    let batcher = DataTableBatcher::new(DataTableBatcherConfig::NEVER).unwrap();
    let tables = batcher.tables();

    let table = create_table();
    let rows: Vec<_> = table.try_to_rows().try_collect().unwrap();

    for _ in 0..3 {
        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        for row in rows.clone() {
            batcher.push_row(row);
        }

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());
    }

    drop(batcher);

    let expected = DataTable::from_rows(
        TableId::ZERO,
        std::iter::repeat_with(|| rows.clone()).take(3).flatten(),
    );

    select! {
            recv(tables) -> batch => {
            let mut table = batch.unwrap();
            // NOTE: Override the resulting table's ID so they can be compared.
            table.table_id = expected.table_id;

            similar_asserts::assert_eq!(expected, table);
        }
        default(std::time::Duration::from_millis(50)) => {
            panic!("output channel never yielded any table");
        }
    }

    assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
}

#[test]
fn num_bytes_trigger() {
    let table = create_table();
    let rows: Vec<_> = table.try_to_rows().try_collect().unwrap();
    let flush_duration = std::time::Duration::from_millis(50);
    let flush_num_bytes = rows
        .iter()
        .take(rows.len() - 1)
        .map(|row| row.total_size_bytes())
        .sum::<u64>();

    let batcher = DataTableBatcher::new(DataTableBatcherConfig {
        flush_num_bytes,
        flush_tick: flush_duration,
        ..DataTableBatcherConfig::NEVER
    })
    .unwrap();
    let tables = batcher.tables();

    assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

    for row in table.try_to_rows() {
        batcher.push_row(row.unwrap());
    }

    // Expect all rows except for the last one (num_bytes trigger).
    select! {
            recv(tables) -> batch => {
            let table = batch.unwrap();
            let expected = DataTable::from_rows(
                table.table_id,
                rows.clone().into_iter().take(rows.len() - 1),
            );
            similar_asserts::assert_eq!(expected, table);
        }
        default(flush_duration) => {
            panic!("output channel never yielded any table");
        }
    }

    // Expect just the last row (duration trigger).
    select! {
            recv(tables) -> batch => {
            let table = batch.unwrap();
            let expected = DataTable::from_rows(
                table.table_id,
                rows.last().cloned(),
            );
            similar_asserts::assert_eq!(expected, table);
        }
        default(flush_duration * 2) => {
            panic!("output channel never yielded any table");
        }
    }

    assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

    drop(batcher);

    assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
}

#[test]
fn num_rows_trigger() {
    let table = create_table();
    let rows: Vec<_> = table.try_to_rows().try_collect().unwrap();
    let flush_duration = std::time::Duration::from_millis(50);
    let flush_num_rows = rows.len() as u64 - 1;

    let batcher = DataTableBatcher::new(DataTableBatcherConfig {
        flush_num_rows,
        flush_tick: flush_duration,
        ..DataTableBatcherConfig::NEVER
    })
    .unwrap();
    let tables = batcher.tables();

    assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

    for row in table.try_to_rows() {
        batcher.push_row(row.unwrap());
    }

    // Expect all rows except for the last one.
    select! {
            recv(tables) -> batch => {
            let table = batch.unwrap();
            let expected = DataTable::from_rows(
                table.table_id,
                rows.clone().into_iter().take(rows.len() - 1),
            );
            similar_asserts::assert_eq!(expected, table);
        }
        default(flush_duration) => {
            panic!("output channel never yielded any table");
        }
    }

    // Expect just the last row.
    select! {
            recv(tables) -> batch => {
            let table = batch.unwrap();
            let expected = DataTable::from_rows(
                table.table_id,
                rows.last().cloned(),
            );
            similar_asserts::assert_eq!(expected, table);
        }
        default(flush_duration * 2) => {
            panic!("output channel never yielded any table");
        }
    }

    assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

    drop(batcher);

    assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
}

#[test]
fn duration_trigger() {
    let table = create_table();
    let rows: Vec<_> = table.try_to_rows().try_collect().unwrap();

    let flush_duration = std::time::Duration::from_millis(50);

    let batcher = DataTableBatcher::new(DataTableBatcherConfig {
        flush_tick: flush_duration,
        ..DataTableBatcherConfig::NEVER
    })
    .unwrap();
    let tables = batcher.tables();

    assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

    _ = std::thread::Builder::new().spawn({
        let mut rows = rows.clone();
        let batcher = batcher.clone();
        move || {
            for row in rows.drain(..rows.len() - 1) {
                batcher.push_row(row);
            }

            std::thread::sleep(flush_duration * 2);

            let row = rows.last().cloned().unwrap();
            batcher.push_row(row);
        }
    });

    // Expect all rows except for the last one.
    select! {
            recv(tables) -> batch => {
            let table = batch.unwrap();
            let expected = DataTable::from_rows(
                table.table_id,
                rows.clone().into_iter().take(rows.len() - 1),
            );
            similar_asserts::assert_eq!(expected, table);
        }
        default(flush_duration * 2) => {
            panic!("output channel never yielded any table");
        }
    }

    // Expect just the last row.
    select! {
            recv(tables) -> batch => {
            let table = batch.unwrap();
            let expected = DataTable::from_rows(
                table.table_id,
                rows.last().cloned(),
            );
            similar_asserts::assert_eq!(expected, table);
        }
        default(flush_duration * 4) => {
            panic!("output channel never yielded any table");
        }
    }

    assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

    drop(batcher);

    assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
}

fn create_table() -> DataTable {
    let timepoint = |frame_nr: i64| {
        TimePoint::from([
            (Timeline::log_time(), Time::now().into()),
            (Timeline::new_sequence("frame_nr"), frame_nr.into()),
        ])
    };

    let row0 = {
        let num_instances = 2;
        let positions: &[Position2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
        let colors: &[_] = &[Color::from_rgb(128, 128, 128)];
        let labels: &[Text] = &[];

        DataRow::from_cells3_or_panic(
            RowId::random(),
            "a",
            timepoint(1),
            num_instances,
            (positions, colors, labels),
        )
    };

    let row1 = {
        let num_instances = 0;
        let colors: &[Color] = &[];

        DataRow::from_cells1_or_panic(RowId::random(), "b", timepoint(1), num_instances, colors)
    };

    let row2 = {
        let num_instances = 1;
        let colors: &[_] = &[Color::from_rgb(255, 255, 255)];
        let labels: &[_] = &[Text("hey".into())];

        DataRow::from_cells2_or_panic(
            RowId::random(),
            "c",
            timepoint(2),
            num_instances,
            (colors, labels),
        )
    };

    let mut table = DataTable::from_rows(TableId::ZERO, [row0, row1, row2]);
    table.compute_all_size_bytes();
    table
}
