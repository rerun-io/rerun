use itertools::Itertools;
use re_log_types::field_types::{ColorRGBA, Point2D};
use re_query::dataframe_util::{df_builder1, view_builder1, view_builder2};
use re_query::{iter_column, visit_component, visit_components2};

#[test]
fn basic_single_iter() {
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
    ];

    let df = df_builder1(&points).unwrap();

    let results = itertools::izip!(points.iter(), iter_column::<Point2D>(&df)).collect_vec();
    assert_eq!(results.len(), 2);
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn iter_struct_with_null() {
    let points = vec![
        None,
        Some(Point2D { x: 1.0, y: 2.0 }),
        None,
        Some(Point2D { x: 3.0, y: 4.0 }),
        None,
    ];

    let df = df_builder1(&points).unwrap();

    let results = itertools::izip!(points.iter(), iter_column::<Point2D>(&df)).collect_vec();
    assert_eq!(results.len(), 5);
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn iter_primitive_with_null() {
    let points = vec![
        None,
        Some(ColorRGBA(0xff000000)),
        Some(ColorRGBA(0x00ff0000)),
        None,
    ];

    let df = df_builder1(&points).unwrap();

    let results = itertools::izip!(points.iter(), iter_column::<ColorRGBA>(&df)).collect_vec();
    assert_eq!(results.len(), 4);
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn single_visit() {
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
        Some(Point2D { x: 5.0, y: 6.0 }),
        Some(Point2D { x: 7.0, y: 8.0 }),
    ];

    let entity_view = view_builder1(&points).unwrap();

    let mut points_out = Vec::<Option<Point2D>>::new();

    visit_component(&entity_view, |point: &Point2D| {
        points_out.push(Some(point.clone()));
    });

    assert_eq!(points, points_out);
}

#[test]
fn joint_visit() {
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
        Some(Point2D { x: 5.0, y: 6.0 }),
        Some(Point2D { x: 7.0, y: 8.0 }),
    ];

    let colors = vec![
        None,
        Some(ColorRGBA(0xff000000)),
        Some(ColorRGBA(0x00ff0000)),
        None,
    ];

    let entity_view = view_builder2(&points, &colors).unwrap();

    let mut points_out = Vec::<Option<Point2D>>::new();
    let mut colors_out = Vec::<Option<ColorRGBA>>::new();

    visit_components2(
        &entity_view,
        |point: &Point2D, color: Option<&ColorRGBA>| {
            points_out.push(Some(point.clone()));
            colors_out.push(color.cloned());
        },
    );

    assert_eq!(points, points_out);
    assert_eq!(colors, colors_out);
}
