use crate::node;

use super::{Node, Position, Quadtree};

impl<T: Position> Quadtree<T> {
    pub fn cover(&mut self, value: &T) {
        let x = value.x();
        let y = value.y();

        assert!(!f32::is_nan(x), "Encountered NaN value for x");
        assert!(!f32::is_nan(y), "Encountered NaN value for y");

        let mut x0 = self.x0;
        let mut y0 = self.y0;
        let mut x1 = self.x1;
        let mut y1 = self.y1;

        if f32::is_nan(x0) {
            x0 = x.floor();
            x1 = x0 + 1.0;
            y0 = y.floor();
            y1 = y0 + 1.0;
        } else {
            // Otherwise, double repeatedly to cover.
            let mut z = if (x1 - x0).is_sign_positive() {
                (x1 - x0)
            } else {
                1.0
            };

            let node = if matches!(self.root(), Some(&Node::Internal(_))) {
                &mut self.root
            } else {
                &mut None
            };

            while x0 > x || x >= x1 || y0 > y || y >= y1 {
                let i = ((y < y0) as usize) << 1 | ((x < x0) as usize);

                let mut parent = [None, None, None, None];
                parent[i] = node.take();
                *node = Some(Box::new(Node::Internal(parent)));

                z *= 2.0;
                match i {
                    0 => {
                        x1 = x0 + z;
                        y1 = y0 + z;
                    }
                    1 => {
                        x0 = x1 - z;
                        y1 = y0 + z;
                    }
                    2 => {
                        x1 = x0 + z;
                        y0 = y1 - z;
                    }
                    3 => {
                        x0 = x1 - z;
                        y0 = y1 - z;
                    }
                    _ => unreachable!(),
                }
            }
        }

        self.x0 = x0;
        self.y0 = y0;
        self.x1 = x1;
        self.y1 = y1;
    }
}

#[cfg(test)]
mod test {
    use crate::quadtree::LeafNode;

    use super::*;

    #[test]
    fn sets_a_trivial_extent_if_the_extent_was_undefined() {
        let mut q = Quadtree::<[f32; 2]>::default();
        q.cover(&[1., 2.]);
        assert_eq!(q.extent(), ([1., 2.], [2., 3.]));
    }

    #[test]
    fn sets_a_non_trivial_squarified_and_centered_extent_if_the_extent_was_trivial() {
        let mut q = Quadtree::<[f32; 2]>::default();
        q.cover(&[0., 0.]);
        q.cover(&[1., 2.]);
        assert_eq!(q.extent(), ([0., 0.], [4., 4.]));
    }

    #[test]
    #[should_panic(expected = "Encountered NaN value for x")]
    fn ignores_panics_on_invalid_points() {
        let mut q = Quadtree::<[f32; 2]>::default();
        q.cover(&[0., 0.]);
        q.cover(&[f32::NAN, 2.]);
    }

    #[test]
    fn repeatedly_doubles_the_existing_extent_if_the_extent_was_non_trivial() {
        fn cover_multiple(q: &mut Quadtree<[f32; 2]>, ps: &[[f32; 2]]) {
            for p in ps {
                q.cover(p);
            }
        }

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [-1., -1.]]);
        assert_eq!(q.extent(), ([-4., -4.], [4., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [1., -1.]]);
        assert_eq!(q.extent(), ([0., -4.], [8., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [3., -1.]]);
        assert_eq!(q.extent(), ([0., -4.], [8., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [3., 1.]]);
        assert_eq!(q.extent(), ([0., 0.], [4., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [3., 3.]]);
        assert_eq!(q.extent(), ([0., 0.], [4., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [1., 3.]]);
        assert_eq!(q.extent(), ([0., 0.], [4., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [-1., 3.]]);
        assert_eq!(q.extent(), ([-4., 0.], [4., 8.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [-1., 1.]]);
        assert_eq!(q.extent(), ([-4., 0.], [4., 8.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [-3., -3.]]);
        assert_eq!(q.extent(), ([-4., -4.], [4., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [3., -3.]]);
        assert_eq!(q.extent(), ([0., -4.], [8., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [5., -3.]]);
        assert_eq!(q.extent(), ([0., -4.], [8., 4.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [5., 3.]]);
        assert_eq!(q.extent(), ([0., 0.], [8., 8.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [5., 5.]]);
        assert_eq!(q.extent(), ([0., 0.], [8., 8.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [3., 5.]]);
        assert_eq!(q.extent(), ([0., 0.], [8., 8.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [-3., 5.]]);
        assert_eq!(q.extent(), ([-4., 0.], [4., 8.]));

        let mut q = Quadtree::<[f32; 2]>::default();
        cover_multiple(&mut q, &[[0., 0.], [2., 2.], [-3., 3.]]);
        assert_eq!(q.extent(), ([-4., 0.], [4., 8.]));
    }

    #[test]
    fn repeatedly_wraps_the_root_node_if_it_has_children() {
        let mut q = Quadtree::<[f32; 2]>::default();
        q.add([0., 0.]);
        q.add([2., 2.]);

        let mut tmp = q.clone();
        tmp.cover(&[3., 3.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap(),
            [
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ]
        ));

        let mut tmp = q.clone();
        tmp.cover(&[-1., 3.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[1]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));

        let mut tmp = q.clone();
        tmp.cover(&[3., -1.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[2]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));

        let mut tmp = q.clone();
        tmp.cover(&[-1., -1.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[3]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));

        let mut tmp = q.clone();
        tmp.cover(&[5., 5.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[0]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));

        let mut tmp = q.clone();
        tmp.cover(&[-3., 5.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[1]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));

        let mut tmp = q.clone();
        tmp.cover(&[5., -3.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[2]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));

        let mut tmp = q.clone();
        tmp.cover(&[-3., -3.]);
        assert!(matches!(
            tmp.root().unwrap().children().unwrap()[3]
                .unwrap()
                .children(),
            Some([
                Some(&Node::Leaf(LeafNode { data: [0., 0.], .. })),
                None,
                None,
                Some(&Node::Leaf(LeafNode { data: [2., 2.], .. })),
            ])
        ));
    }

    #[test]
    fn does_not_wrap_root_node_if_it_is_a_leaf() {
        fn test_point<'a>(mut q: Quadtree<[f32; 2]>, p: [f32; 2]) {
            q.cover(&p);
            assert!(matches!(
                q.root(),
                Some(Node::Leaf(LeafNode { data: [2., 2.], .. }))
            ));
        }

        let mut q = Quadtree::<[f32; 2]>::default();
        q.cover(&[0., 0.]);
        q.add([2., 2.]);
        assert!(matches!(
            q.root(),
            Some(Node::Leaf(LeafNode { data: [2., 2.], .. }))
        ));
        test_point(q.clone(), [3., 3.]);
        test_point(q.clone(), [-1., 3.]);
        test_point(q.clone(), [3., -1.]);
        test_point(q.clone(), [-1., -1.]);
        test_point(q.clone(), [5., 5.]);
        test_point(q.clone(), [-3., 5.]);
        test_point(q.clone(), [5., -3.]);
        test_point(q.clone(), [-3., -3.]);
    }

    #[test]
    fn does_not_wrap_root_node_if_it_is_undefined() {
        fn cover_root(mut q: Quadtree<[f32; 2]>, p: [f32; 2]) -> Option<Box<Node<[f32; 2]>>> {
            q.cover(&p);
            q.root
        }

        let mut q = Quadtree::<[f32; 2]>::default();
        q.cover(&[0., 0.]);
        q.cover(&[2., 2.]);
        assert!(q.root().is_none());
        assert_eq!(cover_root(q.clone(), [3., 3.]), None);
        assert_eq!(cover_root(q.clone(), [-1., 3.]), None);
        assert_eq!(cover_root(q.clone(), [3., -1.]), None);
        assert_eq!(cover_root(q.clone(), [-1., -1.]), None);
        assert_eq!(cover_root(q.clone(), [5., 5.]), None);
        assert_eq!(cover_root(q.clone(), [-3., 5.]), None);
        assert_eq!(cover_root(q.clone(), [5., -3.]), None);
        assert_eq!(cover_root(q.clone(), [-3., -3.]), None);
    }

    #[test]
    fn does_not_crash_on_huge_values() {
        let mut q = Quadtree::<[f32; 2]>::default();
        q.add([1e23, 0.]);
    }
}
