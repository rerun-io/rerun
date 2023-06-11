use re_types::{components, datatypes, Archetype as _};

// TODO: issues for TODOs below
//
// TODO(cmc): include registry tests once registry gets quoted too
// TODO(cmc): include roundtrip tests once serialization is ready
// TODO(cmc): generate all of this (introduce aliases for rust)

#[test]
fn build() {
    let fqname = re_types::archetypes::Points2D::name();
    assert_eq!("rerun.archetypes.Points2D", fqname);

    let points = {
        #[derive(Clone)]
        enum Point2DArray {
            From(Vec<components::Point2D>),
            FromTuples(Vec<(f32, f32)>),
            FromArrays(Vec<[f32; 2]>),
            FromVec2Ds(Vec<datatypes::Vec2D>),
            FromVec2s(Vec<glam::Vec2>),
        }

        impl From<Point2DArray> for Vec<components::Point2D> {
            fn from(points: Point2DArray) -> Self {
                match points {
                    Point2DArray::From(points) => points.into_iter().map(Into::into).collect(),
                    Point2DArray::FromTuples(points) => {
                        points.into_iter().map(Into::into).collect()
                    }
                    Point2DArray::FromArrays(points) => {
                        points.into_iter().map(Into::into).collect()
                    }
                    Point2DArray::FromVec2Ds(points) => {
                        points.into_iter().map(Into::into).collect()
                    }
                    Point2DArray::FromVec2s(points) => points.into_iter().map(Into::into).collect(),
                }
            }
        }

        [
            Point2DArray::From(vec![
                components::Point2D::new(42.0, 666.0),
                components::Point2D::new(0.0, 0.0),
                components::Point2D::new(1.2, 3.4),
            ]),
            Point2DArray::FromTuples(vec![(42.0, 666.0), (0.0, 0.0), (1.2, 3.4)]),
            Point2DArray::FromArrays(vec![[42.0, 666.0], [0.0, 0.0], [1.2, 3.4]]),
            Point2DArray::FromVec2Ds(vec![
                datatypes::Vec2D::new(42.0, 666.0),
                datatypes::Vec2D::new(0.0, 0.0),
                datatypes::Vec2D::new(1.2, 3.4),
            ]),
            Point2DArray::FromVec2s(vec![
                glam::Vec2::new(42.0, 666.0),
                glam::Vec2::new(0.0, 0.0),
                glam::Vec2::new(1.2, 3.4),
            ]),
        ]
    };

    let radii = {
        #[derive(Clone)]
        enum RadiusArray {
            From(Vec<components::Radius>),
            FromF32(Vec<f32>),
        }

        impl From<RadiusArray> for Vec<components::Radius> {
            fn from(radii: RadiusArray) -> Self {
                match radii {
                    RadiusArray::From(radii) => radii.into_iter().map(Into::into).collect(),
                    RadiusArray::FromF32(radii) => radii.into_iter().map(Into::into).collect(),
                }
            }
        }

        [
            RadiusArray::From(vec![
                components::Radius::new(1.0), //
                components::Radius::new(2.0),
                components::Radius::new(3.0),
            ]), //
            RadiusArray::FromF32(vec![1.0, 2.0, 3.0]),
        ]
    };

    // ---

    let expected = re_types::archetypes::Points2D::new([[42.0, 666.0], [0.0, 0.0], [1.2, 3.4]])
        .with_radii([1.0, 2.0, 3.0])
        .with_instance_keys([1, 2, 3])
        .with_colors([[128, 64, 192, 255]])
        .with_labels(["hello", "friend", "o"])
        .with_draw_order(42.0);

    use itertools::Itertools as _;
    for (points, radii) in points.into_iter().cartesian_product(radii) {
        let got = re_types::archetypes::Points2D::new(Vec::<components::Point2D>::from(points))
            .with_radii(Vec::<components::Radius>::from(radii))
            .with_instance_keys([1, 2, 3])
            .with_colors([[128, 64, 192, 255]])
            .with_labels(["hello", "friend", "o"])
            .with_draw_order(42.0);
        assert_eq!(expected, got);
    }
}
