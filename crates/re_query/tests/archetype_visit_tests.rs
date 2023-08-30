use itertools::Itertools;
use re_log_types::RowId;
use re_query::{ArchetypeView, ComponentWithInstances};
use re_types::archetypes::Points2D;
use re_types::components::{Color, InstanceKey, Point2D};

#[test]
fn basic_single_iter() {
    let instance_keys = InstanceKey::from_iter(0..2);
    let points = [
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
    ];

    let component = ComponentWithInstances::from_native(instance_keys, points);

    let results = itertools::izip!(
        points.into_iter(),
        component.values::<Point2D>().unwrap()
    )
    .collect_vec();
    assert_eq!(results.len(), 2);
    results
        .iter()
        .for_each(|(a, b)| assert_eq!(a, b.as_ref().unwrap()));
}

#[test]
fn directly_joined_iter() {
    let instance_keys = InstanceKey::from_iter(0..3);

    let points = [
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
    ];

    let colors = [
        Color::from(0), //
        Color::from(1),
        Color::from(2),
    ];

    let points_comp = ComponentWithInstances::from_native(instance_keys.clone(), points);
    let colors_comp = ComponentWithInstances::from_native(instance_keys, colors);

    let arch_view =
        ArchetypeView::<Points2D>::from_components(RowId::ZERO, [points_comp, colors_comp]);

    let expected_colors = [
        Some(Color::from(0)),
        Some(Color::from(1)),
        Some(Color::from(2)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        arch_view.iter_optional_component::<Color>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn joined_iter_dense_primary() {
    let point_ids = InstanceKey::from_iter(0..3);

    let points = [
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
    ];

    let color_ids = [
        InstanceKey(1), //
        InstanceKey(2),
    ];

    let colors = [
        Color::from(1), //
        Color::from(2),
    ];

    let points_comp = ComponentWithInstances::from_native(point_ids, points);
    let colors_comp = ComponentWithInstances::from_native(color_ids, colors);

    let arch_view =
        ArchetypeView::<Points2D>::from_components(RowId::ZERO, [points_comp, colors_comp]);

    let expected_colors = [None, Some(Color::from(1)), Some(Color::from(2))];

    let results = itertools::izip!(
        expected_colors.iter(),
        arch_view.iter_optional_component::<Color>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn joined_iter_dense_secondary() {
    let point_ids = [
        InstanceKey(0), //
        InstanceKey(2),
        InstanceKey(4),
    ];

    let points = [
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
    ];

    let color_ids = InstanceKey::from_iter(0..5);

    let colors = [
        Color::from(0), //
        Color::from(1),
        Color::from(2),
        Color::from(3),
        Color::from(4),
    ];

    let points_comp = ComponentWithInstances::from_native(point_ids, points);
    let colors_comp = ComponentWithInstances::from_native(color_ids, colors);

    let arch_view =
        ArchetypeView::<Points2D>::from_components(RowId::ZERO, [points_comp, colors_comp]);

    let expected_colors = [
        Some(Color::from(0)), //
        Some(Color::from(2)),
        Some(Color::from(4)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        arch_view.iter_optional_component::<Color>().unwrap()
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
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
        Point2D::new(7.0, 8.0),
    ];

    let color_ids = vec![
        InstanceKey(17), //
        InstanceKey(19),
        InstanceKey(44),
        InstanceKey(96),
        InstanceKey(254),
    ];

    let colors = vec![
        Color::from(17), //
        Color::from(19),
        Color::from(44),
        Color::from(96),
        Color::from(254),
    ];

    let points_comp = ComponentWithInstances::from_native(point_ids, points);
    let colors_comp = ComponentWithInstances::from_native(color_ids, colors);

    let arch_view =
        ArchetypeView::<Points2D>::from_components(RowId::ZERO, [points_comp, colors_comp]);

    let expected_colors = [
        None,
        Some(Color::from(17)), //
        None,
        Some(Color::from(96)),
    ];

    let results = itertools::izip!(
        expected_colors.iter(),
        arch_view.iter_optional_component::<Color>().unwrap()
    )
    .collect_vec();

    assert_eq!(expected_colors.len(), results.len());
    results.iter().for_each(|(a, b)| assert_eq!(*a, b));
}

#[test]
fn single_visit() {
    let instance_keys = InstanceKey::from_iter(0..4);
    let points = [
        Point2D::new(1.0, 2.0),
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
        Point2D::new(7.0, 8.0),
    ];

    let points_comp = ComponentWithInstances::from_native(instance_keys.clone(), points);

    let arch_view = ArchetypeView::<Points2D>::from_components(RowId::ZERO, [points_comp]);

    let mut instance_key_out = Vec::<InstanceKey>::new();
    let mut points_out = Vec::<Point2D>::new();

    itertools::izip!(
        arch_view.iter_instance_keys(),
        arch_view.iter_required_component::<Point2D>().unwrap()
    )
    .for_each(|(inst, point)| {
        instance_key_out.push(inst);
        points_out.push(point);
    });

    assert_eq!(instance_key_out, instance_keys);
    assert_eq!(points.as_slice(), points_out.as_slice());
}

#[test]
fn joint_visit() {
    let points = vec![
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
        Point2D::new(7.0, 8.0),
        Point2D::new(9.0, 10.0),
    ];

    let point_ids = InstanceKey::from_iter(0..5);

    let colors = vec![
        Color::from(0xff000000), //
        Color::from(0x00ff0000),
    ];

    let color_ids = vec![
        InstanceKey(2), //
        InstanceKey(4),
    ];

    let points_comp = ComponentWithInstances::from_native(point_ids, points.clone());
    let colors_comp = ComponentWithInstances::from_native(color_ids, colors);

    let arch_view =
        ArchetypeView::<Points2D>::from_components(RowId::ZERO, [points_comp, colors_comp]);

    let mut points_out = Vec::<Point2D>::new();
    let mut colors_out = Vec::<Option<Color>>::new();

    itertools::izip!(
        arch_view.iter_required_component::<Point2D>().unwrap(),
        arch_view.iter_optional_component::<Color>().unwrap()
    )
    .for_each(|(point, color)| {
        points_out.push(point);
        colors_out.push(color);
    });

    let expected_colors = vec![
        None,
        None,
        Some(Color::from(0xff000000)),
        None,
        Some(Color::from(0x00ff0000)),
    ];

    assert_eq!(points, points_out);
    assert_eq!(expected_colors, colors_out);
}
