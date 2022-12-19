use itertools::Itertools;
use polars_core::export::regex::internal::Inst;
use re_log_types::field_types::{ColorRGBA, Instance, Point2D};
use re_query::dataframe_util::{df_builder1, view_builder1, view_builder2};
use re_query::{iter_column, joined_iter, visit_component, visit_components2};

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
fn trivial_self_joined_iter() {
    let ids = vec![Instance(0), Instance(1)];

    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
    ];

    let entity_view = view_builder1((Some(&ids), &points)).unwrap();

    let results = itertools::izip!(
        points.iter(),
        joined_iter::<Point2D>(&entity_view.primary, &entity_view.primary)
    )
    .collect_vec();
    assert_eq!(results.len(), 2);
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn simple_joined_iter() {
    let point_ids = vec![Instance(0), Instance(2), Instance(4)];

    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
        Some(Point2D { x: 5.0, y: 6.0 }),
    ];

    let color_ids = vec![Instance(0), Instance(1), Instance(3), Instance(4)];

    let colors = vec![
        Some(ColorRGBA(0)),
        Some(ColorRGBA(1)),
        Some(ColorRGBA(3)),
        Some(ColorRGBA(4)),
    ];

    let entity_view =
        view_builder2((Some(&point_ids), &points), (Some(&color_ids), &colors)).unwrap();

    let expected_colors = vec![Some(ColorRGBA(0)), None, Some(ColorRGBA(4))];

    let results = itertools::izip!(
        expected_colors.iter(),
        joined_iter::<ColorRGBA>(&entity_view.primary, &entity_view.components[0])
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn implicit_joined_iter() {
    let points = vec![
        Some(Point2D { x: 1.0, y: 2.0 }),
        Some(Point2D { x: 3.0, y: 4.0 }),
        Some(Point2D { x: 5.0, y: 6.0 }),
    ];

    let color_ids = vec![Instance(1), Instance(2)];

    let colors = vec![Some(ColorRGBA(1)), Some(ColorRGBA(2))];

    let entity_view = view_builder2((None, &points), (Some(&color_ids), &colors)).unwrap();

    let expected_colors = vec![None, Some(ColorRGBA(1)), Some(ColorRGBA(2))];

    let results = itertools::izip!(
        expected_colors.iter(),
        joined_iter::<ColorRGBA>(&entity_view.primary, &entity_view.components[0])
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
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

    let entity_view = view_builder1((None, &points)).unwrap();

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

    let entity_view = view_builder2((None, &points), (None, &colors)).unwrap();

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
