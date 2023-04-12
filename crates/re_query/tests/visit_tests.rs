use itertools::Itertools;
use re_log_types::component_types::{ColorRGBA, InstanceKey, Point2D};
use re_query::{ComponentWithInstances, EntityView};

#[test]
fn basic_single_iter() {
    let instance_keys = [
        InstanceKey(0), //
        InstanceKey(1),
    ];
    let points = [
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
    ];

    let component = ComponentWithInstances::from_native(&instance_keys, &points);

    let results = itertools::izip!(
        points.into_iter(),
        component.iter_values::<Point2D>().unwrap()
    )
    .collect_vec();
    assert_eq!(results.len(), 2);
    results
        .iter()
        .for_each(|(a, b)| assert_eq!(a, b.as_ref().unwrap()));
}

#[test]
fn implicit_joined_iter() {
    let instance_keys = [
        InstanceKey(0), //
        InstanceKey(1),
        InstanceKey(2),
    ];

    let points = [
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
    ];

    let colors = [
        ColorRGBA(0), //
        ColorRGBA(1),
        ColorRGBA(2),
    ];

    let entity_view = EntityView::from_native2(
        (&instance_keys, &points), //
        (&instance_keys, &colors),
    );

    let expected_colors = [
        Some(ColorRGBA(0)), //
        Some(ColorRGBA(1)),
        Some(ColorRGBA(2)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        entity_view.iter_component::<ColorRGBA>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn implicit_primary_joined_iter() {
    let point_ids = [
        InstanceKey(0), //
        InstanceKey(1),
        InstanceKey(2),
    ];

    let points = [
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
    ];

    let color_ids = [
        InstanceKey(1), //
        InstanceKey(2),
    ];

    let colors = [
        ColorRGBA(1), //
        ColorRGBA(2),
    ];

    let entity_view = EntityView::from_native2(
        (&point_ids, &points), //
        (&color_ids, &colors),
    );

    let expected_colors = vec![
        None, //
        Some(ColorRGBA(1)),
        Some(ColorRGBA(2)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        entity_view.iter_component::<ColorRGBA>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn implicit_component_joined_iter() {
    let point_ids = [
        InstanceKey(0), //
        InstanceKey(2),
        InstanceKey(4),
    ];

    let points = [
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
    ];

    let color_ids = [
        InstanceKey(0), //
        InstanceKey(1),
        InstanceKey(2),
        InstanceKey(3),
        InstanceKey(4),
    ];

    let colors = [
        ColorRGBA(0), //
        ColorRGBA(1),
        ColorRGBA(2),
        ColorRGBA(3),
        ColorRGBA(4),
    ];

    let entity_view = EntityView::from_native2(
        (&point_ids, &points), //
        (&color_ids, &colors),
    );

    let expected_colors = vec![
        Some(ColorRGBA(0)), //
        Some(ColorRGBA(2)),
        Some(ColorRGBA(4)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        entity_view.iter_component::<ColorRGBA>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn complex_joined_iter() {
    let point_ids = vec![
        InstanceKey(0), //
        InstanceKey(17),
        InstanceKey(42),
        InstanceKey(96),
    ];

    let points = vec![
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
        Point2D { x: 7.0, y: 8.0 },
    ];

    let color_ids = vec![
        InstanceKey(17), //
        InstanceKey(19),
        InstanceKey(44),
        InstanceKey(96),
        InstanceKey(254),
    ];

    let colors = vec![
        ColorRGBA(17), //
        ColorRGBA(19),
        ColorRGBA(44),
        ColorRGBA(96),
        ColorRGBA(254),
    ];

    let entity_view = EntityView::from_native2(
        (&point_ids, &points), //
        (&color_ids, &colors),
    );

    let expected_colors = vec![
        None,
        Some(ColorRGBA(17)), //
        None,
        Some(ColorRGBA(96)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        entity_view.iter_component::<ColorRGBA>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn single_visit() {
    let instance_keys = [
        InstanceKey(0), //
        InstanceKey(1),
        InstanceKey(2),
        InstanceKey(3),
    ];
    let points = [
        Point2D { x: 1.0, y: 2.0 },
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
        Point2D { x: 7.0, y: 8.0 },
    ];

    let entity_view = EntityView::from_native((&instance_keys, &points));

    let mut instance_key_out = Vec::<InstanceKey>::new();
    let mut points_out = Vec::<Point2D>::new();

    entity_view
        .visit1(|instance_key: InstanceKey, point: Point2D| {
            instance_key_out.push(instance_key);
            points_out.push(point);
        })
        .ok()
        .unwrap();

    assert_eq!(instance_key_out, instance_keys);
    assert_eq!(points.as_slice(), points_out.as_slice());
}

#[test]
fn joint_visit() {
    let points = vec![
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
        Point2D { x: 7.0, y: 8.0 },
        Point2D { x: 9.0, y: 10.0 },
    ];

    let point_ids = [
        InstanceKey(0), //
        InstanceKey(1),
        InstanceKey(2),
        InstanceKey(3),
        InstanceKey(4),
    ];

    let colors = vec![
        ColorRGBA(0xff000000), //
        ColorRGBA(0x00ff0000),
    ];

    let color_ids = vec![
        InstanceKey(2), //
        InstanceKey(4),
    ];

    let entity_view = EntityView::from_native2(
        (&point_ids, &points), //
        (&color_ids, &colors),
    );

    let mut points_out = Vec::<Point2D>::new();
    let mut colors_out = Vec::<Option<ColorRGBA>>::new();

    entity_view
        .visit2(|_: InstanceKey, point: Point2D, color: Option<ColorRGBA>| {
            points_out.push(point);
            colors_out.push(color);
        })
        .ok()
        .unwrap();

    let expected_colors = vec![
        None,
        None,
        Some(ColorRGBA(0xff000000)),
        None,
        Some(ColorRGBA(0x00ff0000)),
    ];

    assert_eq!(points, points_out);
    assert_eq!(expected_colors, colors_out);
}
