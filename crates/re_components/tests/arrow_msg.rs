use re_components::datagen::{build_frame_nr, build_some_colors, build_some_positions2d};
use re_log_types::{ArrowMsg, DataRow, DataTable, RowId};

#[test]
fn arrow_msg_roundtrip() {
    let row = DataRow::from_cells2(
        RowId::random(),
        "world/rects",
        [build_frame_nr(0.into())],
        1,
        (build_some_positions2d(1), build_some_colors(1)),
    )
    .unwrap();

    let table_in = {
        let mut table = row.into_table();
        table.compute_all_size_bytes();
        table
    };
    let msg_in = table_in.to_arrow_msg().unwrap();
    let buf = rmp_serde::to_vec(&msg_in).unwrap();
    let msg_out: ArrowMsg = rmp_serde::from_slice(&buf).unwrap();
    let table_out = {
        let mut table = DataTable::from_arrow_msg(&msg_out).unwrap();
        table.compute_all_size_bytes();
        table
    };

    assert_eq!(msg_in, msg_out);
    assert_eq!(table_in, table_out);
}
