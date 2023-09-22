use re_log_types::{DataReadError, DataRow, DataRowError, EntityPath, RowId, TimePoint};
use re_types::{
    components::{Color, Position2D, Text},
    Loggable as _,
};

#[test]
fn data_row_error_num_instances() {
    let row_id = RowId::ZERO;
    let timepoint = TimePoint::timeless();

    let num_instances = 2;
    let positions: &[Position2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
    let colors: &[_] = &[Color::from_rgb(128, 128, 128)];
    let labels: &[Text] = &[];

    // 0 = clear: legal
    DataRow::from_cells1(row_id, "a/b/c", timepoint.clone(), num_instances, labels).unwrap();

    // 1 = splat: legal
    DataRow::from_cells1(row_id, "a/b/c", timepoint.clone(), num_instances, colors).unwrap();

    // num_instances = standard: legal
    DataRow::from_cells1(row_id, "a/b/c", timepoint.clone(), num_instances, positions).unwrap();

    // anything else is illegal
    let positions: &[Position2D] = &[
        [10.0, 10.0].into(),
        [20.0, 20.0].into(),
        [30.0, 30.0].into(),
    ];
    let err =
        DataRow::from_cells1(row_id, "a/b/c", timepoint, num_instances, positions).unwrap_err();

    match err {
        DataRowError::DataRead(DataReadError::WrongNumberOfInstances {
            entity_path,
            component,
            expected_num_instances,
            num_instances,
        }) => {
            assert_eq!(EntityPath::from("a/b/c"), entity_path);
            assert_eq!(Position2D::name(), component);
            assert_eq!(2, expected_num_instances);
            assert_eq!(3, num_instances);
        }
        _ => unreachable!(),
    }
}

#[test]
fn data_row_error_duped_components() {
    let row_id = RowId::ZERO;
    let timepoint = TimePoint::timeless();

    let positions: &[Position2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];

    let err =
        DataRow::from_cells2(row_id, "a/b/c", timepoint, 2, (positions, positions)).unwrap_err();

    match err {
        DataRowError::DataRead(DataReadError::DupedComponent {
            entity_path,
            component,
        }) => {
            assert_eq!(EntityPath::from("a/b/c"), entity_path);
            assert_eq!(Position2D::name(), component);
        }
        _ => unreachable!(),
    }
}
